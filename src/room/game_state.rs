use serde::{Deserialize, Serialize};

use crate::operation::{Operation, OperationResult};

#[derive(Debug, Clone, Serialize)]
pub struct GameStateResp {
    status: GameState,
    users: Vec<UserState>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GameState {
    Start,
    Wait(String),
    AutoMove,
    End,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct UserState {
    pub id: String,
    pub name: String,
    pub location: UserLocationSequence,
    pub should_move: bool,
    pub moves: Vec<Operation>,
    pub moves_result: Vec<OperationResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct UserLocationSequence {
    pub index: usize,       // 1-12/1-18
    pub child_index: usize, // 0-3
}

impl UserLocationSequence {
    pub fn new(index: usize, child_index: usize) -> Self {
        UserLocationSequence { index, child_index }
    }
    pub fn next(
        &mut self,
        delta: usize,
        max: usize,
        all: &[UserLocationSequence],
    ) -> UserLocationSequence {
        let mut new_index = self.index + delta;
        if new_index > max {
            new_index -= max;
        }
        let new_child_index = all.iter().filter(|s| s.index == new_index).count();

        UserLocationSequence::new(new_index, new_child_index)
    }
}

pub struct UserMove {
    pub op: Operation,
}
