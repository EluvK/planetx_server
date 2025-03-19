use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::room::Room;

pub struct State {
    pub users: HashMap<String, User>, // socket_id -> User
    pub rooms: HashMap<String, Room>, // room_id -> Room
}

impl State {
    fn new() -> Self {
        State {
            users: HashMap::new(),
            rooms: HashMap::new(),
        }
    }

    pub fn upsert_user(&mut self, user: User, socket_id: String) {
        self.users.insert(socket_id, user);
    }

    pub fn check_auth(&self, socket_id: &str) -> Option<&User> {
        self.users.get(socket_id)
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
