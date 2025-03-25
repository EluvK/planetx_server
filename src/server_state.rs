use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use socketioxide::extract::SocketRef;
use tokio::sync::Mutex;
use tracing::info;

use crate::{
    map::{SectorType, validate_index_in_range},
    operation::{Operation, OperationResult},
    room::{GameStateResp, RoomUserOperation, ServerGameState, UserState},
};

type RoomId = String;

pub struct State {
    pub users: HashMap<String, User>,               // socket_id -> User
    pub game_state: HashMap<RoomId, GameStateResp>, // room_id -> room/game_state data
    pub map_data: HashMap<RoomId, ServerGameState>, // map_seed -> map_data
}

enum InnerRoomOp<'a> {
    Enter(&'a String),
    Leave(&'a String),
    LeaveAll,
}
impl State {
    fn new() -> Self {
        State {
            users: HashMap::new(),
            game_state: HashMap::new(),
            map_data: HashMap::new(),
        }
    }

    pub fn upsert_user(&mut self, user: User, socket_id: String) {
        self.users.insert(socket_id, user);
    }

    pub fn check_auth(&self, socket_id: &str) -> Option<&User> {
        self.users.get(socket_id)
    }

    pub fn handle_action_op(
        &mut self,
        user: User,
        operation: &Operation,
    ) -> anyhow::Result<OperationResult> {
        let room_id = self
            .game_state
            .iter()
            .find_map(|(id, gs)| gs.users.iter().any(|u| u.id == user.id).then_some(id))
            .ok_or_else(|| anyhow::anyhow!("user not in any room"))?;
        let game_state = self
            .game_state
            .get(room_id)
            .ok_or_else(|| anyhow::anyhow!("game state not found"))?;
        let map = self
            .map_data
            .get(room_id)
            .ok_or_else(|| anyhow::anyhow!("map not found"))?;
        let op_result = match operation {
            Operation::Survey(s) => {
                if !validate_index_in_range(
                    game_state.start_index,
                    game_state.end_index,
                    s.start,
                    Some(s.end),
                    map.map.size(),
                ) {
                    return Err(anyhow::anyhow!("invalid index"));
                }
                if s.sector_type == SectorType::X {
                    return Err(anyhow::anyhow!("invalid sector type"));
                }
                OperationResult::Survey(map.map.survey_sector(s.start, s.end, &s.sector_type))
            }
            Operation::Target(t) => {
                if !validate_index_in_range(1, map.map.size(), t.index, None, map.map.size()) {
                    return Err(anyhow::anyhow!("invalid index"));
                }
                OperationResult::Target(map.map.target_sector(t.index))
            }
            Operation::Research(r) => {
                OperationResult::Research(map.research_clues[r.index - 1].clone())
            }
            Operation::Locate(l) => OperationResult::Locate(map.map.locate_x(
                l.index,
                &l.pre_sector_type,
                &l.next_sector_type,
            )),
            Operation::ReadyPublish(rp) => {
                // update game state
                OperationResult::ReadyPublish(rp.sectors.len())
            }
            Operation::DoPublish(dp) => {
                // update game state
                OperationResult::DoPublish((dp.index, dp.sector_type.clone()))
            }
        };
        Ok(op_result)
    }

    fn _room_op(&mut self, user: User, op: InnerRoomOp) -> Vec<GameStateResp> {
        let mut res = vec![];
        match op {
            InnerRoomOp::Enter(id) => {
                if let Some(gs) = self.game_state.get_mut(id) {
                    if !gs.users.iter().any(|u| u.id == user.id) && gs.users.len() < 4 {
                        let room_user = UserState::new(&user, gs.users.len() + 1);
                        gs.users.push(room_user);
                        res.push(gs.clone());
                    } else {
                        info!("room full or user already in room");
                    }
                } else {
                    info!("room not found");
                }
            }
            InnerRoomOp::Leave(id) => {
                if let Some(gs) = self.game_state.get_mut(id) {
                    if gs.users.iter().any(|u| u.id == user.id) {
                        gs.users.retain(|u| u.id != user.id);
                        res.push(gs.clone());
                    }
                } else {
                    info!("room not found");
                }
            }
            InnerRoomOp::LeaveAll => {
                for (_, gs) in self.game_state.iter_mut() {
                    if gs.users.iter().any(|u| u.id == user.id) {
                        gs.users.retain(|u| u.id != user.id);
                        res.push(gs.clone());
                    }
                }
            }
        }
        res
    }

    pub fn handle_room_op(
        &mut self,
        socket: SocketRef,
        user: User,
        room_op: RoomUserOperation,
    ) -> anyhow::Result<Vec<GameStateResp>> {
        match room_op {
            RoomUserOperation::Create => {
                let mut results = self._room_op(user.clone(), InnerRoomOp::LeaveAll);
                socket.leave_all();
                let rand_new_id = loop {
                    let rand_id: String =
                        uuid::Uuid::new_v4().to_string().chars().take(4).collect();
                    if !self.game_state.contains_key(&rand_id) {
                        break rand_id;
                    }
                };
                info!("new room id: {}", rand_new_id);

                self.game_state
                    .insert(rand_new_id.clone(), GameStateResp::new(rand_new_id.clone()));
                results.extend(self._room_op(user, InnerRoomOp::Enter(&rand_new_id)));
                socket.join(rand_new_id);
                Ok(results)
            }
            RoomUserOperation::Edit(new_info) => {
                let gs = self
                    .game_state
                    .get_mut(&new_info.room_id)
                    .ok_or_else(|| anyhow::anyhow!("room not found"))?;
                gs.map_seed = new_info.map_seed;
                gs.map_type = new_info.map_type;
                Ok(vec![gs.clone().into()])
            }
            RoomUserOperation::Join(id) => {
                let mut results = self._room_op(user.clone(), InnerRoomOp::LeaveAll);
                socket.leave_all();
                results.extend(self._room_op(user, InnerRoomOp::Enter(&id)));
                socket.join(id);
                Ok(results)
            }
            RoomUserOperation::Leave(id) => {
                socket.leave(id.clone());
                Ok(self._room_op(user, InnerRoomOp::Leave(&id)))
            }
            RoomUserOperation::Prepare(id) => {
                let gs = self
                    .game_state
                    .get_mut(&id)
                    .ok_or_else(|| anyhow::anyhow!("room not found"))?;
                let user = gs
                    .users
                    .iter_mut()
                    .find(|u| u.id == user.id)
                    .ok_or_else(|| anyhow::anyhow!("user not found"))?;
                user.ready = true;
                Ok(vec![gs.clone().into()])
            }
            RoomUserOperation::Unprepare(id) => {
                let gs = self
                    .game_state
                    .get_mut(&id)
                    .ok_or_else(|| anyhow::anyhow!("room not found"))?;
                let user = gs
                    .users
                    .iter_mut()
                    .find(|u| u.id == user.id)
                    .ok_or_else(|| anyhow::anyhow!("user not found"))?;
                user.ready = false;
                Ok(vec![gs.clone().into()])
            }
        }
    }
}

pub fn create_state() -> Arc<Mutex<State>> {
    Arc::new(Mutex::new(State::new()))
}

pub type StateRef = Arc<Mutex<State>>;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct User {
    pub id: String, // some rand uuid for each device.
    pub name: String,
}
