mod game_state;
pub use game_state::*;

use serde::{Deserialize, Serialize};

use crate::{map::MapType, server_state::User};

#[derive(Debug, Clone)]
pub struct Room {
    pub id: String, // some rand id for each room. first 4 chars of uuid.
    pub users: Vec<RoomUser>,
    pub map_seed: u64,
    pub map_type: MapType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoomUserOperation {
    Create,
    Edit(EditRoomInfo),
    Join(String),
    Leave(String),
    Prepare(String),
    Unprepare(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct EditRoomInfo {
    pub room_id: String,
    pub map_type: MapType,
    pub map_seed: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomResult {
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
impl From<Room> for RoomResult {
    fn from(room: Room) -> Self {
        RoomResult {
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
        let edit = RoomUserOperation::Edit(EditRoomInfo {
            room_id: "123".to_string(),
            map_type: MapType::Expert,
            map_seed: 123,
        });

        let str = serde_json::to_string(&create).unwrap();
        assert_eq!(str, r#""create""#);

        let str = serde_json::to_string(&join).unwrap();
        assert_eq!(str, r#"{"join":"room_id"}"#);

        let str = serde_json::to_string(&edit).unwrap();
        assert_eq!(
            str,
            r#"{"edit":{"room_id":"123","map_type":"expert","map_seed":123}}"#
        );
    }
}
