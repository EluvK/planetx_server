use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::{
    map::{MapType, SectorType, validate_index_in_range},
    operation::{Operation, OperationResult},
    room::{GameStateResp, Room, RoomResult, RoomUserOperation, ServerGameState},
};

type RoomId = String;

pub struct State {
    pub users: HashMap<String, User>,               // socket_id -> User
    pub rooms: HashMap<RoomId, Room>,               // room_id -> Room
    pub game_state: HashMap<RoomId, GameStateResp>, // room_id -> game_state
    pub map_data: HashMap<RoomId, ServerGameState>, // map_seed -> map_data
}

enum InnerRoomOp {
    Enter(String),
    Leave(String),
    LeaveAll,
}
impl State {
    fn new() -> Self {
        State {
            users: HashMap::new(),
            rooms: HashMap::new(),
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

    fn user_room(&self, user: &User) -> Option<(&RoomId, &Room)> {
        self.rooms
            .iter()
            .find(|(_, room)| room.users.iter().any(|u| u.id == user.id))
    }

    pub fn handle_action_op(
        &mut self,
        user: User,
        operation: Operation,
    ) -> anyhow::Result<OperationResult> {
        let (room_id, _room) = self
            .user_room(&user)
            .ok_or_else(|| anyhow::anyhow!("user not in room"))?;
        let map = self
            .map_data
            .get(room_id)
            .ok_or_else(|| anyhow::anyhow!("map not found"))?;
        let game_state = self
            .game_state
            .get(room_id)
            .ok_or_else(|| anyhow::anyhow!("game state not found"))?;
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
                OperationResult::DoPublish((dp.index, dp.sector_type))
            }
        };
        Ok(op_result)
    }

    fn _room_op(&mut self, user: User, op: InnerRoomOp) -> Vec<RoomResult> {
        let mut res = vec![];
        match op {
            InnerRoomOp::Enter(id) => {
                if let Some(room) = self.rooms.get_mut(&id) {
                    if !room.users.iter().any(|u| u.id == user.id) && room.users.len() < 4 {
                        let room_user = user.into();
                        room.users.push(room_user);
                        res.push(room.clone().into());
                    }
                }
            }
            InnerRoomOp::Leave(id) => {
                if let Some(room) = self.rooms.get_mut(&id) {
                    if room.users.iter().any(|u| u.id == user.id) {
                        room.users.retain(|u| u.id != user.id);
                        res.push(room.clone().into());
                    }
                }
            }
            InnerRoomOp::LeaveAll => {
                for (_, room) in self.rooms.iter_mut() {
                    if room.users.iter().any(|u| u.id == user.id) {
                        room.users.retain(|u| u.id != user.id);
                        res.push(room.clone().into());
                    }
                }
            }
        }
        res
    }

    pub fn handle_room_op(
        &mut self,
        user: User,
        room_op: RoomUserOperation,
    ) -> anyhow::Result<Vec<RoomResult>> {
        match room_op {
            RoomUserOperation::Create => {
                let rand_new_id = loop {
                    let rand_id: String =
                        uuid::Uuid::new_v4().to_string().chars().take(4).collect();
                    if !self.rooms.contains_key(&rand_id) {
                        break rand_id;
                    }
                };
                let room = Room {
                    id: rand_new_id.clone(),
                    users: vec![user.clone().into()],
                    map_seed: rand::random(), // todo
                    map_type: MapType::Standard,
                };
                self.rooms.insert(rand_new_id.clone(), room.clone());
                let mut results = self._room_op(user.clone(), InnerRoomOp::LeaveAll);
                results.extend(self._room_op(user, InnerRoomOp::Enter(rand_new_id)));
                Ok(results)
            }
            RoomUserOperation::Edit(new_info) => {
                let room = self
                    .rooms
                    .get_mut(&new_info.room_id)
                    .ok_or_else(|| anyhow::anyhow!("room not found"))?;
                room.map_seed = new_info.map_seed;
                room.map_type = new_info.map_type;
                Ok(vec![room.clone().into()])
            }
            RoomUserOperation::Join(id) => {
                let mut results = self._room_op(user.clone(), InnerRoomOp::LeaveAll);
                results.extend(self._room_op(user, InnerRoomOp::Enter(id)));
                Ok(results)
            }
            RoomUserOperation::Leave(id) => Ok(self._room_op(user, InnerRoomOp::Leave(id))),
            RoomUserOperation::Prepare(id) => {
                let room = self
                    .rooms
                    .get_mut(&id)
                    .ok_or_else(|| anyhow::anyhow!("room not found"))?;
                let user = room
                    .users
                    .iter_mut()
                    .find(|u| u.id == user.id)
                    .ok_or_else(|| anyhow::anyhow!("user not found"))?;
                user.ready = true;
                Ok(vec![room.clone().into()])
            }
            RoomUserOperation::Unprepare(id) => {
                let room = self
                    .rooms
                    .get_mut(&id)
                    .ok_or_else(|| anyhow::anyhow!("room not found"))?;
                let user = room
                    .users
                    .iter_mut()
                    .find(|u| u.id == user.id)
                    .ok_or_else(|| anyhow::anyhow!("user not found"))?;
                user.ready = false;
                Ok(vec![room.clone().into()])
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
