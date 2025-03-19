use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::{
    operation::{Operation, OperationResult},
    room::{GameStateResp, Room, RoomResp, RoomUserOperation, ServerGameState},
};

pub struct State {
    pub users: HashMap<String, User>, // socket_id -> User
    pub rooms: HashMap<String, Room>, // room_id -> Room
    pub game_state: HashMap<String, GameStateResp>,
    pub map_data: HashMap<String, ServerGameState>, // map_seed -> map_data
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

    pub fn handle_action_op(
        &mut self,
        user: User,
        operation: Operation,
    ) -> anyhow::Result<Option<OperationResult>> {
        todo!();
        // match operation {
        //     Operation::Survey(s) => Ok(Some(OperationResult::Survey(1))),
        //     Operation::Target(t) => Ok(Some(OperationResult::Target())),
        //     Operation::Research(r) => Ok(Some(OperationResult::Research("ABCDEFX1X2".into()))),
        //     Operation::Locate(l) => Ok(Some(OperationResult::Locate(true))),
        //     Operation::ReadyPublish(rp) => Ok(Some(OperationResult::ReadyPublish(1))),
        //     Operation::DoPublish(dp) => Ok(Some(OperationResult::DoPublish((1, "space".into())))),
        // }
    }

    pub fn handle_room_op(
        &mut self,
        user: User,
        room_op: RoomUserOperation,
    ) -> anyhow::Result<Option<RoomResp>> {
        match room_op {
            RoomUserOperation::Create => {
                let rand_id = loop {
                    let rand_id: String =
                        uuid::Uuid::new_v4().to_string().chars().take(4).collect();
                    if !self.rooms.contains_key(&rand_id) {
                        break rand_id;
                    }
                };
                let room = Room {
                    id: rand_id.clone(),
                    map_seed: rand::random(), // todo
                    users: vec![user.into()],
                };

                self.rooms.insert(rand_id, room.clone());
                Ok(Some(room.into()))
            }
            RoomUserOperation::Join(id) => {
                let room = self
                    .rooms
                    .get_mut(&id)
                    .ok_or_else(|| anyhow::anyhow!("room not found"))?;
                if !room.users.iter().any(|u| u.id == user.id) {
                    let room_user = user.into();
                    room.users.push(room_user);
                }
                Ok(Some(RoomResp {
                    room_id: id,
                    users: room.users.clone(),
                }))
            }
            RoomUserOperation::Leave(id) => {
                let room = self
                    .rooms
                    .get_mut(&id)
                    .ok_or_else(|| anyhow::anyhow!("room not found"))?;
                room.users.retain(|u| u.id != user.id);
                Ok(None)
            }
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
                Ok(Some(room.clone().into()))
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
                Ok(Some(room.clone().into()))
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
