use serde::{Deserialize, Serialize};

use crate::server_state::User;

#[derive(Debug)]
pub struct Room {
    pub id: String, // some rand id for each room.
    pub map_seed: u64,
    pub users: Vec<User>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoomUserOperation {
    Create,
    Join(String),
    Leave(String),
    Prepare,
    Unprepare,
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
