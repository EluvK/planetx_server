use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use socketioxide::extract::SocketRef;
use tokio::sync::Mutex;
use tracing::info;

use crate::{
    map::{SectorType, validate_index_in_range},
    operation::{Operation, OperationResult},
    room::{GameStage, GameState, GameStateResp, RoomUserOperation, ServerGameState, UserState},
};

type RoomId = String;

pub struct State {
    pub users: HashMap<String, (SocketRef, User)>, // socket_id -> User
    pub state_data: HashMap<RoomId, (GameStateResp, ServerGameState)>, // room_id -> game_data
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
            state_data: HashMap::new(),
        }
    }

    pub fn iter_game_state(&self) -> impl Iterator<Item = (&String, &GameStateResp)> {
        self.state_data.iter().map(|(k, v)| (k, &v.0))
    }
    // pub fn iter_server_state(&self) -> impl Iterator<Item = (&String, &ServerGameState)> {
    //     self.state_data.iter().map(|(k, v)| (k, &v.1))
    // }

    pub fn iter_mut_game_state(&mut self) -> impl Iterator<Item = (&String, &mut GameStateResp)> {
        self.state_data.iter_mut().map(|(k, v)| (k, &mut v.0))
    }
    // pub fn iter_mut_server_state(&mut self) -> impl Iterator<Item = (&String, &mut ServerGameState)> {
    //     self.state_data.iter_mut().map(|(k, v)| (k, &mut v.1))
    // }

    pub fn iter_mut_all(
        &mut self,
    ) -> impl Iterator<Item = (&String, (&mut GameStateResp, &mut ServerGameState))> {
        self.state_data
            .iter_mut()
            .map(|(k, v)| (k, (&mut v.0, &mut v.1)))
    }

    pub fn get_game_state(&mut self, room_id: &str) -> Option<&mut GameStateResp> {
        self.state_data.get_mut(room_id).map(|(gs, _)| gs)
    }

    // pub fn get_server_state(&mut self, room_id: &str) -> Option<&mut ServerGameState> {
    //     self.state_data.get_mut(room_id).map(|(_, map)| map)
    // }

    pub fn get_state(
        &mut self,
        room_id: &str,
    ) -> Option<(&mut GameStateResp, &mut ServerGameState)> {
        self.state_data.get_mut(room_id).map(|(gs, map)| (gs, map))
    }

    pub fn upsert_user(&mut self, socket_id: String, user: User, socket: SocketRef) {
        self.users.insert(socket_id, (socket, user));
    }

    pub fn check_auth(&self, socket_id: &str) -> Option<&User> {
        self.users.get(socket_id).map(|(_, user)| user)
    }

    pub fn handle_action_op(
        &mut self,
        user: User,
        operation: &Operation,
    ) -> anyhow::Result<OperationResult> {
        let room_id = self
            .iter_game_state()
            .find_map(|(id, gs)| gs.users.iter().any(|u| u.id == user.id).then_some(id))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("user not in any room"))?;
        let (game_state, map) = self.get_state(&room_id).ok_or_else(|| {
            anyhow::anyhow!("game state or map not found for room id: {}", room_id)
        })?;
        if !game_state.check_waiting_for(&user.id) {
            return Err(anyhow::anyhow!("not user turn"));
        }

        match (operation, &game_state.game_stage) {
            (
                Operation::Survey(_)
                | Operation::Target(_)
                | Operation::Research(_)
                | Operation::Locate(_),
                GameStage::UserMove,
            ) => {}
            (Operation::ReadyPublish(_), GameStage::MeetingProposal) => {}
            (Operation::DoPublish(_), GameStage::MeetingPublish) => {}
            _rest => {
                return Err(anyhow::anyhow!(
                    "invalid operation at {:?} stage",
                    &game_state.game_stage
                ));
            }
        }

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
                let range_size = if s.start <= s.end {
                    s.end - s.start
                } else {
                    s.end + map.map.size() - s.start
                };
                game_state.user_move(&user.id, 4 - range_size / 3)?;
                OperationResult::Survey(map.map.survey_sector(s.start, s.end, &s.sector_type))
            }
            Operation::Target(t) => {
                if !validate_index_in_range(
                    game_state.start_index,
                    game_state.end_index,
                    t.index,
                    None,
                    map.map.size(),
                ) {
                    return Err(anyhow::anyhow!("invalid index"));
                }
                game_state.user_move(&user.id, 4)?;
                OperationResult::Target(map.map.target_sector(t.index))
            }
            Operation::Research(r) => {
                // todo add can not reasearch continously limit
                game_state.user_move(&user.id, 1)?;
                OperationResult::Research(
                    map.research_clues
                        .iter()
                        .find(|c| c.index == r.index)
                        .cloned()
                        .ok_or_else(|| anyhow::anyhow!("clue not found"))?,
                )
            }
            Operation::Locate(l) => {
                game_state.user_move(&user.id, 5)?;
                // todo add game last phase logic
                OperationResult::Locate(map.map.locate_x(
                    l.index,
                    &l.pre_sector_type,
                    &l.next_sector_type,
                ))
            }
            Operation::ReadyPublish(rp) => {
                // update game state
                // todo check sector count and type.
                map.ready_publish_token(&user.id, &rp.sectors)?;
                OperationResult::ReadyPublish(rp.sectors.len())
            }
            Operation::DoPublish(dp) => {
                // update game state
                map.publish_token(&user.id, dp.index, &dp.sector_type)?;
                OperationResult::DoPublish((dp.index, dp.sector_type.clone()))
            }
        };

        match operation {
            Operation::ReadyPublish(_) | Operation::DoPublish(_) => {}
            op => {
                game_state.user_operation_record(&user.id, op, &op_result)?;
            }
        }

        Ok(op_result)
    }

    fn _room_op(&mut self, user: User, op: InnerRoomOp) -> Vec<GameStateResp> {
        let mut res = vec![];
        match op {
            InnerRoomOp::Enter(id) => {
                if let Some(gs) = self.get_game_state(id) {
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
                if let Some(gs) = self.get_game_state(id) {
                    if gs.users.iter().any(|u| u.id == user.id) {
                        gs.users.retain(|u| u.id != user.id);
                        res.push(gs.clone());
                    }
                } else {
                    info!("room not found");
                }
            }
            InnerRoomOp::LeaveAll => {
                for (_, gs) in self.iter_mut_game_state() {
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
                    if !self.iter_game_state().any(|(id, _)| id == &rand_id) {
                        break rand_id;
                    }
                };
                info!("new room id: {}", rand_new_id);

                self.state_data.insert(
                    rand_new_id.clone(),
                    (
                        GameStateResp::new(rand_new_id.clone()),
                        ServerGameState::placeholder(),
                    ),
                );
                results.extend(self._room_op(user, InnerRoomOp::Enter(&rand_new_id)));
                socket.join(rand_new_id);
                Ok(results)
            }
            RoomUserOperation::Edit(new_info) => {
                let gs = self
                    .get_game_state(&new_info.room_id)
                    .ok_or_else(|| anyhow::anyhow!("room not found"))?;
                gs.map_seed = new_info.map_seed;
                gs.map_type = new_info.map_type;
                gs.end_index = gs.map_type.sector_count() / 2;
                Ok(vec![gs.clone()])
            }
            RoomUserOperation::Join(id) => {
                let gs = self
                    .get_game_state(&id)
                    .ok_or_else(|| anyhow::anyhow!("room not found"))?;
                if gs.status != GameState::NotStarted {
                    return Err(anyhow::anyhow!("room already started"));
                }
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
                    .get_game_state(&id)
                    .ok_or_else(|| anyhow::anyhow!("room not found"))?;
                let user = gs
                    .users
                    .iter_mut()
                    .find(|u| u.id == user.id)
                    .ok_or_else(|| anyhow::anyhow!("user not found"))?;
                user.ready = true;
                Ok(vec![gs.clone()])
            }
            RoomUserOperation::Unprepare(id) => {
                let gs = self
                    .get_game_state(&id)
                    .ok_or_else(|| anyhow::anyhow!("room not found"))?;
                let user = gs
                    .users
                    .iter_mut()
                    .find(|u| u.id == user.id)
                    .ok_or_else(|| anyhow::anyhow!("user not found"))?;
                user.ready = false;
                Ok(vec![gs.clone()])
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
