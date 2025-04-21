use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServerResp {
    Version(String),
    RejoinRoom(String),
    RoomErrors(RoomError),
    OpErrors(OpError),
    RecommendErrors(RecommendError),
}

impl ServerResp {
    pub fn auth_success_version() -> Self {
        Self::Version("0.0.6".to_string())
    }

    pub fn rejoin_room(room_id: String) -> Self {
        Self::RejoinRoom(room_id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoomError {
    RoomNotFound,
    RoomStarted,
    RoomFull,
    UserNotFoundInRoom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpError {
    UserNotFoundInRoom,
    GameNotFound,

    NotUsersTurn,
    InvalidMoveInStage,
    InvalidIndex,
    InvalidClue,
    InvalidSectorType,
    InvalidIndexOfPrime,
    TokenNotEnough,

    SectorAlreadyRevealed,
    TargetTimeExhausted,
    ResearchContiuously,

    EndGameCanNotLocate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecommendError {
    UserNotFoundInRoom,
    GameNotFound,

    NotEnoughData,
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_serde() {
        let e1 = ServerResp::RoomErrors(RoomError::RoomNotFound);
        let s = serde_json::to_string(&e1).unwrap();
        assert_eq!(s, r#"{"room_errors":"room_not_found"}"#);

        let e2 = ServerResp::RejoinRoom("room_id".to_string());
        let s = serde_json::to_string(&e2).unwrap();
        assert_eq!(s, r#"{"rejoin_room":"room_id"}"#);
    }
}
