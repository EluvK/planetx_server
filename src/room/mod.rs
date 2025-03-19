mod game_state;
pub use game_state::*;

use serde::{Deserialize, Serialize};

use crate::server_state::User;

#[derive(Debug, Clone)]
pub struct Room {
    pub id: String, // some rand id for each room. first 4 chars of uuid.
    pub map_seed: u64,
    pub users: Vec<RoomUser>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoomUserOperation {
    Create,
    Join(String),
    Leave(String),
    Prepare(String),
    Unprepare(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomResp {
    pub room_id: String,
    pub users: Vec<RoomUser>, // user names
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomUser {
    pub id: String,
    pub name: String,
    pub ready: bool,
}

impl From<User> for RoomUser {
    fn from(user: User) -> Self {
        RoomUser {
            id: user.id,
            name: user.name,
            ready: false,
        }
    }
}
impl From<Room> for RoomResp {
    fn from(room: Room) -> Self {
        RoomResp {
            room_id: room.id,
            users: room.users,
        }
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_room_op_serde() {
        let create = RoomUserOperation::Create;
        let join = RoomUserOperation::Join("room_id".to_string());

        let str = serde_json::to_string(&create).unwrap();
        assert_eq!(str, r#""create""#);

        let str = serde_json::to_string(&join).unwrap();
        assert_eq!(str, r#"{"join":"room_id"}"#);
    }
}
