use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use socketioxide::extract::SocketRef;
use tokio::sync::Mutex;
use tracing::info;

use crate::{
    map::{SectorType, validate_index_in_range},
    operation::{Operation, OperationResult},
    recommendation::{RecommendOperation, RecommendOperationResult},
    room::{
        GameStage, GameState, GameStateResp, OpError, RecommendError, RoomError, RoomUserOperation,
        ServerGameState, ServerResp, UserState,
    },
};

type RoomId = String;

pub struct State {
    pub users: HashMap<String, (SocketRef, User)>, // socket_id -> User
    pub state_data: HashMap<RoomId, (GameStateResp, ServerGameState)>, // room_id -> game_data
}

enum InnerRoomOp<'a> {
    Enter(&'a String),
    Leave(&'a String),
    LeaveAll,
    EnableBot(&'a String),
    DisableBot(&'a String),
}
impl State {
    fn new() -> Self {
        State {
            users: HashMap::new(),
            state_data: HashMap::new(),
        }
    }

    pub fn iter_game_state(&self) -> impl Iterator<Item = (&String, &GameStateResp)> {
        self.state_data.iter().map(|(k, v)| (k, &v.0))
    }
    pub fn iter_all(&self) -> impl Iterator<Item = (&String, (&GameStateResp, &ServerGameState))> {
        self.state_data.iter().map(|(k, v)| (k, (&v.0, &v.1)))
    }
    // pub fn iter_server_state(&self) -> impl Iterator<Item = (&String, &ServerGameState)> {
    //     self.state_data.iter().map(|(k, v)| (k, &v.1))
    // }

    pub fn iter_mut_game_state(&mut self) -> impl Iterator<Item = (&String, &mut GameStateResp)> {
        self.state_data.iter_mut().map(|(k, v)| (k, &mut v.0))
    }
    // pub fn iter_mut_server_state(&mut self) -> impl Iterator<Item = (&String, &mut ServerGameState)> {
    //     self.state_data.iter_mut().map(|(k, v)| (k, &mut v.1))
    // }

    pub fn iter_mut_all(
        &mut self,
    ) -> impl Iterator<Item = (&String, (&mut GameStateResp, &mut ServerGameState))> {
        self.state_data
            .iter_mut()
            .map(|(k, v)| (k, (&mut v.0, &mut v.1)))
    }

    pub fn get_game_state(&mut self, room_id: &str) -> Option<&mut GameStateResp> {
        self.state_data.get_mut(room_id).map(|(gs, _)| gs)
    }

    // pub fn get_server_state(&mut self, room_id: &str) -> Option<&mut ServerGameState> {
    //     self.state_data.get_mut(room_id).map(|(_, map)| map)
    // }

    pub fn get_state(
        &mut self,
        room_id: &str,
    ) -> Option<(&mut GameStateResp, &mut ServerGameState)> {
        self.state_data.get_mut(room_id).map(|(gs, map)| (gs, map))
    }

    pub fn upsert_user(&mut self, socket_id: String, user: User, socket: SocketRef) {
        self.iter_game_state().for_each(|(room_id, gs)| {
            if gs.users.iter().any(|u| u.id == user.id) {
                info!("upsert user: {} in room: {}", user.id, room_id);
                socket.leave_all();
                socket
                    .emit("server_resp", &ServerResp::rejoin_room(room_id.clone()))
                    .ok();
                socket.join(room_id.clone());
            }
        });
        self.users.insert(socket_id, (socket, user));
    }

    pub fn check_auth(&self, socket_id: &str) -> Option<&User> {
        self.users.get(socket_id).map(|(_, user)| user)
    }

    pub fn handle_action_op(
        &mut self,
        user: User,
        operation: &Operation,
    ) -> Result<OperationResult, OpError> {
        // ) -> anyhow::Result<OperationResult> {
        let room_id = self
            .iter_game_state()
            .find_map(|(id, gs)| gs.users.iter().any(|u| u.id == user.id).then_some(id))
            .cloned()
            .ok_or(OpError::UserNotFoundInRoom)?;
        let (gs, ss) = self.get_state(&room_id).ok_or(OpError::GameNotFound)?;

        if !gs.check_waiting_for(&user.id) {
            return Err(OpError::NotUsersTurn);
        }

        match (operation, &gs.game_stage) {
            (
                Operation::Survey(_)
                | Operation::Target(_)
                | Operation::Research(_)
                | Operation::Locate(_),
                GameStage::UserMove,
            ) => {}
            (Operation::ReadyPublish(_), GameStage::MeetingProposal) => {}
            (Operation::DoPublish(_), GameStage::MeetingPublish) => {}
            (Operation::DoPublish(_) | Operation::Locate(_), GameStage::LastMove) => {}
            _rest => {
                return Err(OpError::InvalidMoveInStage);
            }
        }

        let op_result = match operation {
            Operation::Survey(s) => {
                if !validate_index_in_range(
                    gs.start_index,
                    gs.end_index,
                    s.start,
                    Some(s.end),
                    ss.map.size(),
                ) {
                    return Err(OpError::InvalidIndex);
                }
                if s.sector_type == SectorType::X {
                    return Err(OpError::InvalidSectorType);
                }
                if s.sector_type == SectorType::Comet
                    && (!matches!(s.start, 2 | 3 | 5 | 7 | 11 | 13 | 17)
                        || !matches!(s.end, 2 | 3 | 5 | 7 | 11 | 13 | 17))
                {
                    return Err(OpError::InvalidIndexOfPrime);
                }
                let range_size = if s.start <= s.end {
                    s.end - s.start
                } else {
                    s.end + ss.map.size() - s.start
                };
                gs.user_move(&user.id, 4 - range_size / 3)?;
                OperationResult::Survey(ss.map.survey_sector(s.start, s.end, &s.sector_type))
            }
            Operation::Target(t) => {
                let user_state = gs
                    .users
                    .iter_mut()
                    .find(|u| u.id == user.id)
                    .ok_or(OpError::UserNotFoundInRoom)?;
                if user_state
                    .moves
                    .iter()
                    .filter(|op| matches!(op, Operation::Target(_)))
                    .count()
                    >= 2
                {
                    return Err(OpError::TargetTimeExhausted);
                }
                if !validate_index_in_range(
                    gs.start_index,
                    gs.end_index,
                    t.index,
                    None,
                    ss.map.size(),
                ) {
                    return Err(OpError::InvalidIndex);
                }
                gs.user_move(&user.id, 4)?;
                OperationResult::Target(ss.map.target_sector(t.index))
            }
            Operation::Research(r) => {
                let user_state = gs
                    .users
                    .iter_mut()
                    .find(|u| u.id == user.id)
                    .ok_or(OpError::UserNotFoundInRoom)?;
                if user_state
                    .moves
                    .last()
                    .is_some_and(|op| matches!(op, Operation::Research(_)))
                {
                    return Err(OpError::ResearchContiuously);
                }
                gs.user_move(&user.id, 1)?;
                OperationResult::Research(
                    ss.research_clues
                        .iter()
                        .find(|c| c.index == r.index)
                        .cloned()
                        .ok_or(OpError::InvalidClue)?,
                )
            }
            Operation::Locate(l) => {
                if ss.terminator_location.is_some() {
                    // or we can use game_stage == GameStage::LastMove
                    let user_state = gs
                        .users
                        .iter_mut()
                        .find(|u| u.id == user.id)
                        .ok_or(OpError::UserNotFoundInRoom)?;
                    if !user_state.can_locate {
                        return Err(OpError::EndGameCanNotLocate);
                    }
                    user_state.can_locate = false;
                    user_state.last_move = false;
                    OperationResult::Locate(ss.map.locate_x(
                        l.index,
                        &l.pre_sector_type,
                        &l.next_sector_type,
                    ))
                } else {
                    gs.user_move(&user.id, 5)?;
                    let r = OperationResult::Locate(ss.map.locate_x(
                        l.index,
                        &l.pre_sector_type,
                        &l.next_sector_type,
                    ));

                    if matches!(r, OperationResult::Locate(true)) {
                        gs.game_stage = GameStage::LastMove;
                        let terminator = gs
                            .users
                            .iter_mut()
                            .find(|u| u.id == user.id)
                            .ok_or(OpError::UserNotFoundInRoom)?;
                        terminator.last_move = false;
                        let terminator_location = terminator.location.clone();
                        gs.users.iter_mut().for_each(|user| {
                            user.last_move = user.location.index_lt(&terminator_location);
                        });
                        ss.terminator_location = Some(terminator_location);
                    }
                    r
                }
            }
            Operation::ReadyPublish(rp) => {
                ss.ready_publish_token(&user.id, &rp.sectors)?;
                OperationResult::ReadyPublish(rp.sectors.len())
            }
            Operation::DoPublish(dp) => {
                if ss.revealed_sector_indexs.contains(&dp.index) {
                    return Err(OpError::SectorAlreadyRevealed);
                }

                match &ss.terminator_location {
                    Some(terminator_location) => {
                        let user_state = gs
                            .users
                            .iter_mut()
                            .find(|u| u.id == user.id)
                            .ok_or(OpError::UserNotFoundInRoom)?;

                        let before_more_then_4 = user_state.location.index_le4(terminator_location);
                        if user_state.can_locate && before_more_then_4 {
                            // user can either locate or publish twice
                            user_state.can_locate = false;
                        } else {
                            user_state.last_move = false;
                        }
                        ss.last_move_publish_token(&user.id, dp.index, &dp.sector_type)?;
                    }
                    None => {
                        ss.publish_token(&user.id, dp.index, &dp.sector_type)?;
                    }
                }

                OperationResult::DoPublish((dp.index, dp.sector_type.clone()))
            }
        };

        ss.choices
            .get_mut(&user.id)
            .ok_or(OpError::UserNotFoundInRoom)?
            .add_operation(operation.clone(), op_result.clone());
        let user_state = gs
            .users
            .iter_mut()
            .find(|u| u.id == user.id)
            .ok_or(OpError::UserNotFoundInRoom)?;
        match operation {
            Operation::ReadyPublish(_) | Operation::DoPublish(_) => {
                user_state.moves_result.push(op_result.clone());
            }
            op => {
                user_state.moves.push(op.clone());
                user_state.moves_result.push(op_result.clone());
            }
        }

        Ok(op_result)
    }

    fn _room_op(&mut self, user: User, op: InnerRoomOp) -> Vec<GameStateResp> {
        let mut res = vec![];
        match op {
            InnerRoomOp::Enter(id) => {
                if let Some(gs) = self.get_game_state(id) {
                    if !gs.users.iter().any(|u| u.id == user.id) && gs.users.len() < 4 {
                        let room_user = UserState::placeholder(&user, gs.users.len() + 1, false);
                        gs.users.push(room_user);
                        res.push(gs.clone());
                    } else {
                        info!("room full or user already in room");
                    }
                } else {
                    info!("room not found");
                }
            }
            InnerRoomOp::Leave(id) => {
                if let Some(gs) = self.get_game_state(id) {
                    if gs.users.iter().any(|u| u.id == user.id) {
                        gs.users.retain(|u| u.id != user.id);
                        res.push(gs.clone());
                    }
                } else {
                    info!("room not found");
                }
            }
            InnerRoomOp::LeaveAll => {
                for (_, gs) in self.iter_mut_game_state() {
                    if gs.users.iter().any(|u| u.id == user.id) {
                        gs.users.retain(|u| u.id != user.id);
                        res.push(gs.clone());
                    }
                }
            }
            InnerRoomOp::EnableBot(id) => {
                if let Some(gs) = self.get_game_state(id) {
                    if !gs.users.iter().any(|u| u.is_bot) && gs.users.len() < 4 {
                        let bot_user = User {
                            id: "bot".to_string(),
                            name: "protocol".to_string(),
                        };
                        let room_bot_user =
                            UserState::placeholder(&bot_user, gs.users.len() + 1, true);
                        gs.users.push(room_bot_user);
                        res.push(gs.clone());
                    } else {
                        info!("room full or bot already in room");
                    }
                } else {
                    info!("room not found");
                }
            }
            InnerRoomOp::DisableBot(id) => {
                if let Some(gs) = self.get_game_state(id) {
                    if gs.users.iter().any(|u| u.is_bot) {
                        gs.users.retain(|u| !u.is_bot);
                        res.push(gs.clone());
                    }
                } else {
                    info!("room not found");
                }
            }
        }
        res
    }

    pub fn handle_room_op(
        &mut self,
        socket: SocketRef,
        user: User,
        room_op: RoomUserOperation,
    ) -> Result<Vec<GameStateResp>, RoomError> {
        match room_op {
            RoomUserOperation::Create => {
                let mut results = self._room_op(user.clone(), InnerRoomOp::LeaveAll);
                socket.leave_all();
                let rand_new_id = loop {
                    // maybe a pure number id is better
                    let rand_id: String = uuid::Uuid::new_v4()
                        .to_string()
                        .chars()
                        .filter(|c| c.is_ascii_digit())
                        .take(4)
                        .collect();
                    if rand_id.len() == 4 && !self.iter_game_state().any(|(id, _)| id == &rand_id) {
                        break rand_id;
                    }
                };
                info!("new room id: {}", rand_new_id);

                self.state_data.insert(
                    rand_new_id.clone(),
                    (
                        GameStateResp::new(rand_new_id.clone()),
                        ServerGameState::placeholder(),
                    ),
                );
                results.extend(self._room_op(user, InnerRoomOp::Enter(&rand_new_id)));
                socket.join(rand_new_id);
                Ok(results)
            }
            RoomUserOperation::SwitchBot(id) => {
                let gs = self.get_game_state(&id).ok_or(RoomError::RoomNotFound)?;
                if gs.status != GameState::NotStarted {
                    return Err(RoomError::RoomStarted);
                }
                let res = if gs.users.iter().any(|u| u.is_bot) {
                    self._room_op(user, InnerRoomOp::DisableBot(&id))
                } else {
                    if gs.users.len() >= 4 {
                        return Err(RoomError::RoomFull);
                    }
                    self._room_op(user, InnerRoomOp::EnableBot(&id))
                };
                Ok(res)
            }
            RoomUserOperation::Edit(new_info) => {
                let gs = self
                    .get_game_state(&new_info.room_id)
                    .ok_or(RoomError::RoomNotFound)?;
                gs.map_seed = new_info.map_seed;
                gs.map_type = new_info.map_type;
                gs.end_index = gs.map_type.sector_count() / 2;
                Ok(vec![gs.clone()])
            }
            RoomUserOperation::Join(id) => {
                let gs = self.get_game_state(&id).ok_or(RoomError::RoomNotFound)?;
                if gs.status != GameState::NotStarted && !gs.users.iter().any(|u| u.id == user.id) {
                    return Err(RoomError::RoomStarted);
                }
                if gs.users.iter().any(|u| u.id == user.id) {
                    socket.join(id);
                    return Ok(vec![gs.clone()]);
                }
                if gs.users.len() >= 4 {
                    return Err(RoomError::RoomFull);
                }
                let mut results = self._room_op(user.clone(), InnerRoomOp::LeaveAll);
                socket.leave_all();
                results.extend(self._room_op(user, InnerRoomOp::Enter(&id)));
                socket.join(id);
                Ok(results)
            }
            RoomUserOperation::Leave(id) => {
                socket.leave(id.clone());
                Ok(self._room_op(user, InnerRoomOp::Leave(&id)))
            }
            RoomUserOperation::Prepare(id) => {
                let gs = self.get_game_state(&id).ok_or(RoomError::RoomNotFound)?;
                let user = gs
                    .users
                    .iter_mut()
                    .find(|u| u.id == user.id)
                    .ok_or(RoomError::UserNotFoundInRoom)?;
                user.ready = true;
                Ok(vec![gs.clone()])
            }
            RoomUserOperation::Unprepare(id) => {
                let gs = self.get_game_state(&id).ok_or(RoomError::RoomNotFound)?;
                let user = gs
                    .users
                    .iter_mut()
                    .find(|u| u.id == user.id)
                    .ok_or(RoomError::UserNotFoundInRoom)?;
                user.ready = false;
                Ok(vec![gs.clone()])
            }
        }
    }

    pub fn handle_recommend_op(
        &mut self,
        user: User,
        op: RecommendOperation,
    ) -> Result<RecommendOperationResult, RecommendError> {
        let room_id = self
            .iter_game_state()
            .find_map(|(id, gs)| gs.users.iter().any(|u| u.id == user.id).then_some(id))
            .cloned()
            .ok_or(RecommendError::UserNotFoundInRoom)?;
        let (_gs, ss) = self
            .get_state(&room_id)
            .ok_or(RecommendError::GameNotFound)?;
        let choice = ss
            .choices
            .get(&user.id)
            .ok_or(RecommendError::UserNotFoundInRoom)?;
        match op {
            RecommendOperation::Count => {
                if !choice.initialized {
                    return Err(RecommendError::NotEnoughData);
                } else {
                    return Ok(RecommendOperationResult::Count(choice.all.len()));
                }
            }
            RecommendOperation::CanLocate => {
                if !choice.initialized {
                    return Err(RecommendError::NotEnoughData);
                } else {
                    let can_locate = choice.can_locate();
                    return Ok(RecommendOperationResult::CanLocate(can_locate));
                }
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
