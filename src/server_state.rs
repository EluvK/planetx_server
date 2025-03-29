use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use socketioxide::extract::SocketRef;
use tokio::sync::Mutex;
use tracing::info;

use crate::{
    map::{SectorType, validate_index_in_range},
    operation::{Operation, OperationResult},
    room::{GameStage, GameState, GameStateResp, RoomUserOperation, ServerGameState, UserState},
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
        self.users.insert(socket_id, (socket, user));
    }

    pub fn check_auth(&self, socket_id: &str) -> Option<&User> {
        self.users.get(socket_id).map(|(_, user)| user)
    }

    pub fn handle_action_op(
        &mut self,
        user: User,
        operation: &Operation,
    ) -> anyhow::Result<OperationResult> {
        let room_id = self
            .iter_game_state()
            .find_map(|(id, gs)| gs.users.iter().any(|u| u.id == user.id).then_some(id))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("user not in any room"))?;
        let (gs, ss) = self.get_state(&room_id).ok_or_else(|| {
            anyhow::anyhow!("game state or ss not found for room id: {}", room_id)
        })?;
        if !gs.check_waiting_for(&user.id) {
            return Err(anyhow::anyhow!("not user turn"));
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
                return Err(anyhow::anyhow!(
                    "invalid operation at {:?} stage",
                    &gs.game_stage
                ));
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
                    return Err(anyhow::anyhow!("invalid index"));
                }
                if s.sector_type == SectorType::X {
                    return Err(anyhow::anyhow!("invalid sector type"));
                }
                if s.sector_type == SectorType::Comet
                    && (!matches!(s.start, 2 | 3 | 5 | 7 | 11 | 13 | 17)
                        || !matches!(s.end, 2 | 3 | 5 | 7 | 11 | 13 | 17))
                {
                    return Err(anyhow::anyhow!("comet sector type start end must be prime"));
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
                if !validate_index_in_range(
                    gs.start_index,
                    gs.end_index,
                    t.index,
                    None,
                    ss.map.size(),
                ) {
                    return Err(anyhow::anyhow!("invalid index"));
                }
                gs.user_move(&user.id, 4)?;
                OperationResult::Target(ss.map.target_sector(t.index))
            }
            Operation::Research(r) => {
                let user_state = gs
                    .users
                    .iter_mut()
                    .find(|u| u.id == user.id)
                    .ok_or_else(|| anyhow::anyhow!("user not found"))?;
                if user_state
                    .moves
                    .last()
                    .is_some_and(|op| matches!(op, Operation::Research(_)))
                {
                    return Err(anyhow::anyhow!("user can not research continuously"));
                }
                gs.user_move(&user.id, 1)?;
                OperationResult::Research(
                    ss.research_clues
                        .iter()
                        .find(|c| c.index == r.index)
                        .cloned()
                        .ok_or_else(|| anyhow::anyhow!("clue not found"))?,
                )
            }
            Operation::Locate(l) => {
                if ss.terminator_location.is_some() {
                    // or we can use game_stage == GameStage::LastMove
                    let user_state = gs
                        .users
                        .iter_mut()
                        .find(|u| u.id == user.id)
                        .ok_or_else(|| anyhow::anyhow!("user not found"))?;
                    if !user_state.can_locate {
                        return Err(anyhow::anyhow!("user can not locate anymore"));
                    }
                    user_state.can_locate = false;
                    user_state.last_move = false;
                    let r = OperationResult::Locate(ss.map.locate_x(
                        l.index,
                        &l.pre_sector_type,
                        &l.next_sector_type,
                    ));

                    r
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
                            .ok_or_else(|| anyhow::anyhow!("user not found"))?;
                        terminator.last_move = false;
                        let terminator_location = terminator.location.clone();
                        gs.users.iter_mut().for_each(|user| {
                            if user.location.index >= terminator_location.index {
                                user.last_move = false;
                            }
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
                    return Err(anyhow::anyhow!("sector {} already revealed", dp.index));
                }

                if ss.terminator_location.is_some() {
                    let user_state = gs
                        .users
                        .iter_mut()
                        .find(|u| u.id == user.id)
                        .ok_or_else(|| anyhow::anyhow!("user not found"))?;
                    let ti = ss.terminator_location.clone().unwrap().index;
                    let ui = user_state.location.index;
                    let before_ge_4 = (ti > ui && (ui + 4) <= ti)
                        || (ui + 4 <= (ti + gs.map_type.sector_count()));
                    if user_state.can_locate && before_ge_4 {
                        user_state.can_locate = false;
                    } else {
                        user_state.last_move = false;
                    }
                    ss.last_move_publish_token(&user.id, dp.index, &dp.sector_type)?;
                } else {
                }

                ss.publish_token(&user.id, dp.index, &dp.sector_type)?;
                OperationResult::DoPublish((dp.index, dp.sector_type.clone()))
            }
        };

        let user_state = gs
            .users
            .iter_mut()
            .find(|u| u.id == user.id)
            .ok_or_else(|| anyhow::anyhow!("user not found"))?;
        match operation {
            Operation::ReadyPublish(_) | Operation::DoPublish(_) => {
                user_state.moves_result.push(op_result.clone());
            }
            op => {
                user_state.moves.push(op.clone());
                user_state.moves_result.push(op_result.clone());
                // gs.user_operation_record(&user.id, op, &op_result)?;
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
                        let room_user = UserState::new(&user, gs.users.len() + 1);
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
        }
        res
    }

    pub fn handle_room_op(
        &mut self,
        socket: SocketRef,
        user: User,
        room_op: RoomUserOperation,
    ) -> anyhow::Result<Vec<GameStateResp>> {
        match room_op {
            RoomUserOperation::Create => {
                let mut results = self._room_op(user.clone(), InnerRoomOp::LeaveAll);
                socket.leave_all();
                let rand_new_id = loop {
                    let rand_id: String =
                        uuid::Uuid::new_v4().to_string().chars().take(4).collect();
                    if !self.iter_game_state().any(|(id, _)| id == &rand_id) {
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
            RoomUserOperation::Edit(new_info) => {
                let gs = self
                    .get_game_state(&new_info.room_id)
                    .ok_or_else(|| anyhow::anyhow!("room not found"))?;
                gs.map_seed = new_info.map_seed;
                gs.map_type = new_info.map_type;
                gs.end_index = gs.map_type.sector_count() / 2;
                Ok(vec![gs.clone()])
            }
            RoomUserOperation::Join(id) => {
                let gs = self
                    .get_game_state(&id)
                    .ok_or_else(|| anyhow::anyhow!("room not found"))?;
                if gs.status != GameState::NotStarted && !gs.users.iter().any(|u| u.id == user.id) {
                    return Err(anyhow::anyhow!("room already started, can not join"));
                }
                if gs.users.iter().any(|u| u.id == user.id) {
                    socket.join(id);
                    return Ok(vec![gs.clone()]);
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
                let gs = self
                    .get_game_state(&id)
                    .ok_or_else(|| anyhow::anyhow!("room not found"))?;
                let user = gs
                    .users
                    .iter_mut()
                    .find(|u| u.id == user.id)
                    .ok_or_else(|| anyhow::anyhow!("user not found"))?;
                user.ready = true;
                Ok(vec![gs.clone()])
            }
            RoomUserOperation::Unprepare(id) => {
                let gs = self
                    .get_game_state(&id)
                    .ok_or_else(|| anyhow::anyhow!("room not found"))?;
                let user = gs
                    .users
                    .iter_mut()
                    .find(|u| u.id == user.id)
                    .ok_or_else(|| anyhow::anyhow!("user not found"))?;
                user.ready = false;
                Ok(vec![gs.clone()])
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
