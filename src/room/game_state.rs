use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{
    map::{Clue, ClueSecret, Map, MapType, SecretToken, SectorType, Token},
    operation::{Operation, OperationResult},
    room::OpError,
    server_state::User,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct GameStateResp {
    pub id: String, // some rand id for each room. first 4 chars of uuid.
    pub status: GameState,
    pub game_stage: GameStage,
    pub hint: Option<String>,
    pub users: Vec<UserState>,
    pub start_index: usize,
    #[serde(skip)]
    pub round: usize,
    pub end_index: usize,
    pub map_seed: u64,
    pub map_type: MapType,
    pub game_result: Option<Vec<UserResultSummary>>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GameStage {
    UserMove,
    MeetingProposal,
    MeetingPublish,
    MeetingCheck,
    LastMove,
    GameEnd,
}

impl GameStateResp {
    pub fn new(id: String) -> Self {
        GameStateResp {
            id,
            status: GameState::NotStarted,
            game_stage: GameStage::UserMove,
            hint: None,
            users: vec![],
            start_index: 1,
            end_index: 6,
            round: 1,
            map_seed: rand::random::<u32>() as u64,
            map_type: MapType::Standard,
            game_result: None,
        }
    }

    pub fn empty() -> Self {
        GameStateResp {
            id: "".to_string(),
            status: GameState::NotStarted,
            game_stage: GameStage::UserMove,
            hint: None,
            users: vec![],
            start_index: 1,
            end_index: 6,
            round: 1,
            map_seed: 0,
            map_type: MapType::Standard,
            game_result: None,
        }
    }

    pub fn check_waiting_for(&mut self, user_id: &str) -> bool {
        // if status is Wating, and user_id is in the waiting list, return true and delete it from the list.
        if let GameState::Wait(ref mut waiting_list) = self.status {
            if let Some(index) = waiting_list.iter().position(|id| id == user_id) {
                waiting_list.remove(index);
                if waiting_list.is_empty() {
                    self.status = GameState::AutoMove;
                }
                return true;
            }
        }
        false
    }

    pub fn user_move(&mut self, user_id: &str, delta: usize) -> Result<(), OpError> {
        let all = self
            .users
            .iter()
            .map(|u| u.location.clone())
            .collect::<Vec<_>>();
        let user_state = self
            .users
            .iter_mut()
            .find(|u| u.id == user_id)
            .ok_or(OpError::UserNotFoundInRoom)?;
        user_state.location = user_state.location.next(delta, &all);

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GameState {
    NotStarted,
    Starting,
    Wait(Vec<String>),
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
    pub last_move: bool,
    pub can_locate: bool,
    pub moves: Vec<Operation>,
    #[serde(skip)]
    pub moves_result: Vec<OperationResult>,
    pub used_token: Vec<SecretToken>,
}

impl UserState {
    pub fn placeholder(user: &User, child_index: usize) -> Self {
        UserState {
            id: user.id.clone(),
            name: user.name.clone(),
            ready: false,
            location: UserLocationSequence::placeholder(1, child_index),
            last_move: true,
            can_locate: true,
            moves: vec![],
            moves_result: vec![],
            used_token: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserResultSummary {
    pub id: String,
    pub name: String,
    pub sum: usize,
    pub first: usize,
    pub comet: usize,        // 彗星得分
    pub asteroid: usize,     // 小行星得分
    pub dwarf_planet: usize, // 矮行星得分
    pub nebula: usize,       // 星云得分
    pub x: usize,            // x clue
    pub step: usize,         // 终局位置
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct UserLocationSequence {
    pub index: usize,       // 1-12/1-18
    pub child_index: usize, // 1,2,3,4
    #[serde(skip)]
    pub max: usize, // 12/18
    #[serde(skip)]
    pub round: usize, // started at 1. x clue is only for round 1. 0 means every round, then
}

impl UserLocationSequence {
    pub fn placeholder(index: usize, child_index: usize) -> Self {
        UserLocationSequence {
            index,
            child_index,
            max: 12, // default to 12, will be updated in the game state.
            round: 1,
        }
    }
    pub fn new(index: usize, child_index: usize, max: usize) -> Self {
        UserLocationSequence {
            index,
            child_index,
            max,
            round: 1,
        }
    }
    pub fn next(&mut self, delta: usize, all: &[UserLocationSequence]) -> UserLocationSequence {
        let mut result = self.clone();
        result.index = self.index + delta;
        if result.index > result.max {
            result.index -= result.max;
            result.round += 1;
        }
        result.child_index = all.iter().filter(|&s| result.is_some_sector(s)).count() + 1;
        result
    }

    pub fn is_some_sector(&self, other: &UserLocationSequence) -> bool {
        self.index == other.index && self.round == other.round
    }

    pub fn index_lt(&self, other: &UserLocationSequence) -> bool {
        self.round * self.max + self.index < other.round * other.max + other.index
    }
    pub fn index_le4(&self, other: &UserLocationSequence) -> bool {
        self.round * self.max + self.index <= other.round * other.max + other.index - 4
    }

    pub fn step(&self) -> usize {
        (self.round - 1) * self.max + self.index
    }
}

#[derive(Debug, Clone)]
pub struct ServerGameState {
    pub map: Map,
    pub research_clues: Vec<Clue>,
    pub x_clues: Vec<Clue>,
    pub user_tokens: HashMap<String, Vec<Token>>,
    pub terminator_location: Option<UserLocationSequence>,
    pub revealed_sector_indexs: Vec<usize>,
}

impl ServerGameState {
    pub fn placeholder() -> Self {
        ServerGameState {
            map: Map::place_holder(),
            research_clues: vec![],
            x_clues: vec![],
            user_tokens: HashMap::new(),
            terminator_location: None,
            revealed_sector_indexs: vec![],
        }
    }

    pub fn clue_secret(&self) -> Vec<ClueSecret> {
        self.research_clues
            .iter()
            .map(|c| ClueSecret {
                index: c.index.clone(),
                secret: c.as_secret(),
            })
            .chain(self.x_clues.iter().map(|c| ClueSecret {
                index: c.index.clone(),
                secret: c.as_secret(),
            }))
            .collect()
    }

    pub fn ready_publish_token(
        &mut self,
        user_id: &str,
        input_tokens: &[SectorType],
    ) -> Result<(), OpError> {
        let tokens = self
            .user_tokens
            .get_mut(user_id)
            .ok_or(OpError::UserNotFoundInRoom)?;
        let mut edited_tokens = tokens.clone();
        for it in input_tokens {
            edited_tokens
                .iter_mut()
                .find(|t| t.is_not_used(it))
                .ok_or(OpError::TokenNotEnough)?
                .set_to_be_placed();
        }
        *tokens = edited_tokens;
        Ok(())
    }

    pub fn publish_token(
        &mut self,
        user_id: &str,
        index: usize,
        r#type: &SectorType,
    ) -> Result<(), OpError> {
        let tokens = self
            .user_tokens
            .get_mut(user_id)
            .ok_or(OpError::UserNotFoundInRoom)?;
        let mut edited_tokens = tokens.clone();
        edited_tokens
            .iter_mut()
            .find(|t| t.is_ready_published(r#type))
            .ok_or(OpError::TokenNotEnough)?
            .set_published(index);
        *tokens = edited_tokens;
        Ok(())
    }

    pub fn last_move_publish_token(
        &mut self,
        user_id: &str,
        index: usize,
        r#type: &SectorType,
    ) -> Result<(), OpError> {
        // let tokens = self
        //     .user_tokens
        //     .get_mut(user_id)
        //     .ok_or_else(|| anyhow::anyhow!("user not found"))?;
        // let mut edited_tokens = tokens.clone();
        self.user_tokens
            .get_mut(user_id)
            .ok_or(OpError::UserNotFoundInRoom)?
            .iter_mut()
            .find(|t| !t.placed && t.r#type == *r#type)
            .ok_or(OpError::TokenNotEnough)?
            .set_to_be_placed()
            .set_published(index);
        // *tokens = edited_tokens;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_game_state_serde() {
        let mut gs = GameStateResp::empty();
        let json = serde_json::to_string(&gs).unwrap();
        assert_eq!(
            json,
            r#"{"id":"","status":"not_started","game_stage":"user_move","hint":null,"users":[],"start_index":1,"end_index":6,"map_seed":0,"map_type":"standard"}"#
        );

        gs.status = GameState::Wait(vec!["1234".to_string()]);
        let json = serde_json::to_string(&gs).unwrap();
        assert_eq!(
            json,
            r#"{"id":"","status":{"wait":["1234"]},"game_stage":"user_move","hint":null,"users":[],"start_index":1,"end_index":6,"map_seed":0,"map_type":"standard"}"#
        );
    }
}
