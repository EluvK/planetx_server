use serde::{Deserialize, Serialize};

use crate::{
    map::{Clue, Map, MapType},
    operation::{Operation, OperationResult},
    server_state::User,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct GameStateResp {
    pub id: String, // some rand id for each room. first 4 chars of uuid.
    pub status: GameState,
    pub hint: Option<String>,
    pub users: Vec<UserState>,
    pub start_index: usize,
    pub end_index: usize,
    pub map_seed: u64,
    pub map_type: MapType,
}

impl GameStateResp {
    pub fn new(id: String) -> Self {
        GameStateResp {
            id,
            status: GameState::NotStarted,
            hint: None,
            users: vec![],
            start_index: 1,
            end_index: 9,
            map_seed: rand::random(),
            map_type: MapType::Standard,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GameState {
    NotStarted,
    Starting,
    Wait,
    AutoMove,
    End,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct UserState {
    pub id: String,
    pub name: String,
    pub ready: bool,
    pub location: UserLocationSequence,
    pub should_move: bool,
    pub moves: Vec<Operation>,
    pub moves_result: Vec<OperationResult>,
}

impl UserState {
    pub fn new(user: &User, child_index: usize) -> Self {
        UserState {
            id: user.id.clone(),
            name: user.name.clone(),
            ready: false,
            location: UserLocationSequence::new(1, child_index),
            should_move: false,
            moves: vec![],
            moves_result: vec![],
        }
    }
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

#[derive(Debug, Clone)]
pub struct ServerGameState {
    pub map: Map,
    pub research_clues: Vec<Clue>,
    pub x_clues: Vec<Clue>,
}
