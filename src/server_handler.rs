use std::{collections::HashMap, vec};

use crate::{
    operation::Operation,
    room::{
        GameStage, GameState, GameStateResp, RoomUserOperation, ServerGameState,
        UserLocationSequence, UserState,
    },
    server_state::{StateRef, User},
};
use rand::{SeedableRng, rngs::SmallRng, seq::SliceRandom};
use socketioxide::{
    SocketIo,
    extract::{Data, SocketRef, State},
};
use tracing::info;

pub async fn handle_on_connect(_io: SocketIo, socket: SocketRef, _state: State<StateRef>) {
    // let client_id = uuid::Uuid::new_v4().to_string();
    // state
    //     .lock()
    //     .await
    //     .users
    //     .insert(socket.id.to_string(), client_id.clone());
    // socket.extensions.insert::<String>(client_id);

    info!(ns = "socket.io", ?socket.id, "new client connected");

    socket.on(
        "auth",
        |socket: SocketRef, state: State<StateRef>, user: Data<User>| async move {
            state
                .0
                .lock()
                .await
                .upsert_user(socket.id.to_string(), user.0.clone(), socket.clone());
            info!(ns = "socket.io", ?socket.id, "auth {:?}", user.0);
            socket.emit("server_resp", "auth success").ok();
        },
    );

    socket.on_disconnect(|socket: SocketRef, state: State<StateRef>| async move {
        state.0.lock().await.users.remove(socket.id.as_str());
        info!(ns = "socket.io", ?socket.id, "disconnected");
    });

    socket.on(
        "op",
        |io: SocketIo,socket: SocketRef, State::<StateRef>(state), Data::<Operation>(op)| async move {
            handle_op(io, socket, state, op).await;
        },
    );

    socket.on(
        "room",
        |io: SocketIo,
         socket: SocketRef,
         State::<StateRef>(state),
         Data::<RoomUserOperation>(op)| async move {
            handle_room(io, socket, state, op).await;
        },
    );

    socket.on(
        "sync",
        |_io: SocketIo, socket: SocketRef, state: State<StateRef>| async move {
            let user = state.lock().await.check_auth(socket.id.as_str()).cloned();
            let Some(user) = user else {
                info!(ns = "socket.io", ?socket.id, "unauthorized sync");
                return;
            };
            for (_room_id, (gs, ss)) in state.lock().await.iter_all() {
                for user_state in gs.users.iter() {
                    if user_state.id != user.id {
                        continue;
                    }

                    socket.emit("game_start", &ss.clue_secret()).ok();

                    info!(ns = "socket.io", ?socket.id, "sync game state {:?}", gs);
                    socket.emit("game_state", &gs).ok();

                    for re in user_state.moves_result.iter() {
                        socket.emit("op_result", re).ok();
                    }

                    let Some(tokens) = ss.user_tokens.get(&user.id) else {
                        continue;
                    };
                    info!(ns = "socket.io", ?socket.id, "sync tokens {:?}", tokens);
                    socket.emit("token", &tokens).ok();

                    let tokens = ss
                        .user_tokens
                        .iter()
                        .flat_map(|(_user_id, tokens)| tokens.iter())
                        .filter(|t| t.placed)
                        .map(|t| &t.secret)
                        .cloned()
                        .collect::<Vec<_>>();
                    socket.emit("board_tokens", &tokens).ok();
                }
            }
        },
    );
}

async fn handle_op(_io: SocketIo, socket: SocketRef, state: StateRef, op: Operation) {
    let user = state.lock().await.check_auth(socket.id.as_str()).cloned();
    let Some(user) = user else {
        info!(ns = "socket.io", ?socket.id, "unauthorized room op {:?}", op);
        return;
    };

    info!(?op, ?socket.id, "received op {:?}", op);

    match state.lock().await.handle_action_op(user, &op) {
        Ok(resp) => {
            // to the user
            info!(ns = "socket.io", ?socket.id, ?resp, "op success");
            socket.emit("op_result", &resp).ok();
            // to other users in the room
            // the automove will do the broadcast
            // socket.to("room_id").emit("op", &op).await.ok();
        }
        Err(e) => {
            info!(ns = "socket.io", ?socket.id, ?e, "op error");
            socket.emit("server_resp", &format!("op error {e}")).ok();
        }
    }
}

async fn handle_room(_io: SocketIo, socket: SocketRef, state: StateRef, op: RoomUserOperation) {
    let user = state.lock().await.check_auth(socket.id.as_str()).cloned();
    let Some(user) = user else {
        info!(ns = "socket.io", ?socket.id, "unauthorized room op {:?}", op);
        return;
    };

    info!(?op, ?socket.id, "received room op {:?}", op);

    match state
        .lock()
        .await
        .handle_room_op(socket.clone(), user.clone(), op)
    {
        Ok(resp) => {
            let mut do_resp = false;
            for gs in resp {
                info!(ns = "socket.io", ?socket.id, ?gs, "room op success");

                socket.to(gs.id.clone()).emit("game_state", &gs).await.ok();
                if gs.users.iter().any(|u| u.id == user.id) {
                    socket.emit("game_state", &gs).ok();
                    do_resp = true;
                }
            }
            if !do_resp {
                // no game state to response, empty client game state
                socket.emit("game_state", &GameStateResp::empty()).ok();
            }
        }

        Err(e) => {
            info!(ns = "socket.io", ?socket.id, ?e, "room op error");
            socket
                .emit("server_resp", &format!("room op error {e}"))
                .ok();
        }
    }
}

pub fn register_state_manager(state: StateRef, io: SocketIo) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
    tokio::task::spawn(async move {
        loop {
            interval.tick().await;
            let mut state = state.lock().await;

            // 1. clean empty game rooms
            let mut clean_room_ids = Vec::new();
            for (room_id, gs) in state.iter_game_state() {
                if gs.users.is_empty() {
                    clean_room_ids.push(room_id.clone());
                }
            }
            for room_id in clean_room_ids {
                state.state_data.remove(&room_id);
            }

            // 2 check if all users in a room are ready, and start the game
            let mut updated_tokens = Vec::new();
            for (room_id, (gs, ss)) in state.iter_mut_all() {
                if gs.status == GameState::NotStarted && gs.users.iter().all(|u| u.ready) {
                    gs.status = GameState::Starting;
                    // gs.hint = Some("Game is starting".to_string());
                    // broadcast_room_game_state(&io, gs).await;
                    gs.start_index = 1;
                    gs.end_index = gs.map_type.sector_count() / 2;
                    gs.users.shuffle(&mut SmallRng::seed_from_u64(gs.map_seed));
                    let mut user_tokens = HashMap::new();
                    for (index, user) in gs.users.iter_mut().enumerate() {
                        user.location = UserLocationSequence::new(
                            gs.start_index,
                            index + 1,
                            gs.map_type.sector_count(),
                        );
                        let tokens = gs.map_type.generate_tokens(user.id.clone(), index + 1);
                        user_tokens.insert(user.id.clone(), tokens);
                    }

                    gs.hint = Some("Game is starting".to_string());
                    broadcast_room_game_state(&io, gs).await;

                    let rng = SmallRng::seed_from_u64(gs.map_seed);
                    let Ok(map) = crate::map::Map::new(rng, gs.map_type.clone()) else {
                        gs.status = GameState::End;
                        gs.hint = Some("Map generation failed".to_string());
                        broadcast_room_game_state(&io, gs).await;
                        continue;
                    };
                    info!(?map, "map generated");
                    let Ok((research_clues, x_clues)) = crate::map::ClueGenerator::new(
                        gs.map_seed,
                        map.sectors.clone(),
                        map.r#type.clone(),
                    )
                    .generate_clues() else {
                        gs.status = GameState::End;
                        gs.hint = Some("Clue generation failed".to_string());
                        broadcast_room_game_state(&io, gs).await;
                        continue;
                    };
                    let server_game_state = ServerGameState {
                        map,
                        research_clues,
                        x_clues,
                        user_tokens,
                        terminator_location: None,
                        revealed_sector_indexs: vec![],
                    };
                    io.of("/xplanet")
                        .unwrap()
                        .to(room_id.clone())
                        .emit("game_start", &server_game_state.clue_secret())
                        .await
                        .ok();
                    // distrubute tokens emiting to users
                    updated_tokens.push(server_game_state.user_tokens.clone());

                    *ss = server_game_state;

                    gs.status = GameState::AutoMove;
                    gs.hint = Some("Game started".to_string());
                    broadcast_room_game_state(&io, gs).await;
                }
            }
            // send each token to user
            for tokens in &updated_tokens {
                send_each_token(&state, tokens);
            }

            // 3. autoMove as server
            updated_tokens.clear();
            for (room_id, (gs, ss)) in state.iter_mut_all() {
                if gs.status == GameState::AutoMove && gs.game_stage == GameStage::UserMove {
                    // find the first point from gs.start_index, move to it.

                    let Some(next_point) = find_next_point(gs, false) else {
                        gs.status = GameState::End;
                        gs.hint = Some("No more points".to_string());
                        io.of("/xplanet")
                            .unwrap()
                            .to(room_id.clone())
                            .emit("game_state", &gs)
                            .await
                            .ok();
                        continue;
                    };
                    gs.start_index = next_point.index;
                    gs.end_index = next_point.index + gs.map_type.sector_count() / 2 - 1;
                    if gs.end_index > gs.map_type.sector_count() {
                        gs.end_index -= gs.map_type.sector_count();
                    }
                    match next_point.r#type {
                        PointType::User(id) => {
                            let name = gs
                                .users
                                .iter()
                                .find(|u| u.id == id)
                                .map(|u| u.name.clone())
                                .unwrap_or_else(|| "Unknown".to_string());
                            gs.status = GameState::Wait(vec![id]);
                            gs.game_stage = GameStage::UserMove;
                            gs.hint = Some(format!("{} should move", name));
                        }
                        PointType::Meeting => {
                            info!("should start a meeting");
                            gs.status =
                                GameState::Wait(gs.users.iter().map(|u| u.id.clone()).collect());
                            gs.game_stage = GameStage::MeetingProposal;
                            gs.hint = Some("Meeting proposal, Everyone should move".to_string());
                        }
                        PointType::XClue => {
                            info!("should broadcast xclue");
                            let index = gs
                                .map_type
                                .xclue_points()
                                .iter()
                                .position(|(i, c)| {
                                    *i == next_point.index && *c == next_point.child_index
                                })
                                .unwrap_or(0);
                            let xclue = ss.x_clues.get(index).map_or(vec![], |x| vec![x.clone()]);
                            io.of("/xplanet")
                                .unwrap()
                                .to(room_id.clone())
                                .emit("xclue", &xclue)
                                .await
                                .ok();
                            let Some(second_point) = find_next_point(gs, true) else {
                                gs.status = GameState::End;
                                gs.hint = Some("No more points".to_string());
                                io.of("/xplanet")
                                    .unwrap()
                                    .to(room_id.clone())
                                    .emit("game_state", &gs)
                                    .await
                                    .ok();
                                continue;
                            };
                            gs.hint = Some("X clue time".to_string());
                            gs.start_index = second_point.index;
                            gs.end_index = second_point.index + gs.map_type.sector_count() / 2 - 1;
                            if gs.end_index > gs.map_type.sector_count() {
                                gs.end_index -= gs.map_type.sector_count();
                            }
                            gs.game_stage = GameStage::UserMove;
                            gs.status = GameState::AutoMove;
                        }
                    }
                    broadcast_room_game_state(&io, gs).await;
                }

                // meeting check phase
                if gs.status == GameState::AutoMove && gs.game_stage == GameStage::MeetingCheck {
                    let mut result = vec![];
                    let mut checked_tokens = ss
                        .user_tokens
                        .iter_mut()
                        .flat_map(|(user_id, tokens)| {
                            tokens
                                .iter_mut()
                                .filter(|t| t.any_ready_checked())
                                .map(|t| (user_id.clone(), t))
                        })
                        .collect::<Vec<(String, &mut crate::map::Token)>>();
                    // we need to sort the tokens by sector_index, and then check them one by one
                    checked_tokens.sort_by(|(_user_id_a, token_a), (_user_id_b, token_b)| {
                        token_a
                            .secret
                            .sector_index
                            .cmp(&token_b.secret.sector_index)
                    });

                    for (user_id, token) in checked_tokens {
                        let all_users_location = gs
                            .users
                            .iter()
                            .map(|u| u.location.clone())
                            .collect::<Vec<_>>();
                        let user = gs
                            .users
                            .iter_mut()
                            .find(|u| u.id == user_id)
                            .unwrap_or_else(|| panic!("user not found: {user_id}"));
                        if ss
                            .map
                            .meeting_check(token.secret.sector_index, &token.r#type)
                        {
                            // right, reveal the token
                            token.secret.r#type = Some(token.r#type.clone());
                            result.push(format!(
                                "{}'s token at {}, {} is right",
                                user.name, token.secret.sector_index, token.r#type
                            ));
                            ss.revealed_sector_indexs.push(token.secret.sector_index);
                        } else {
                            // punish the user move 1 step, token reveal and move outside the map
                            token.secret.r#type = Some(token.r#type.clone());
                            token.secret.meeting_index = 4;
                            user.location = user.location.next(1, &all_users_location);
                            result.push(format!(
                                "{}'s token at {}, {} is wrong, user move 1 step",
                                user.name, token.secret.sector_index, token.r#type
                            ));
                        }
                    }
                    // next checked tokens
                    let mut double_check_tokens = ss
                        .user_tokens
                        .iter_mut()
                        .flat_map(|(user_id, tokens)| {
                            tokens
                                .iter_mut()
                                .filter(|t| {
                                    t.secret.r#type.is_none()
                                        && t.placed
                                        && ss
                                            .revealed_sector_indexs
                                            .contains(&t.secret.sector_index)
                                })
                                .map(|t| (user_id.clone(), t))
                        })
                        .collect::<Vec<(String, &mut crate::map::Token)>>();
                    double_check_tokens.sort_by(|(_user_id_a, token_a), (_user_id_b, token_b)| {
                        token_a
                            .secret
                            .sector_index
                            .cmp(&token_b.secret.sector_index)
                    });

                    for (user_id, token) in double_check_tokens {
                        let all_users_location = gs
                            .users
                            .iter()
                            .map(|u| u.location.clone())
                            .collect::<Vec<_>>();
                        let user = gs
                            .users
                            .iter_mut()
                            .find(|u| u.id == user_id)
                            .unwrap_or_else(|| panic!("user not found: {user_id}"));
                        if ss
                            .map
                            .meeting_check(token.secret.sector_index, &token.r#type)
                        {
                            // right, reveal the token
                            token.secret.r#type = Some(token.r#type.clone());
                            result.push(format!(
                                "{}'s token at {}, {} is right",
                                user.name, token.secret.sector_index, token.r#type
                            ));
                        } else {
                            // punish the user move 1 step, token reveal and move outside the map
                            token.secret.r#type = Some(token.r#type.clone());
                            token.secret.meeting_index = 4;
                            user.location = user.location.next(1, &all_users_location);
                            result.push(format!(
                                "{}'s token at {}, {} is wrong, user move 1 step",
                                user.name, token.secret.sector_index, token.r#type
                            ));
                        }
                    }

                    info!("meeting check result: {:?}", result);
                    // no one need to publish, go to next user
                    // make waiting next user move
                    gs.status = GameState::AutoMove;
                    gs.game_stage = GameStage::UserMove;
                    gs.hint = Some("Push forward".to_string());
                    // need to find next user to move
                    let Some(second_point) = find_next_point(gs, true) else {
                        gs.status = GameState::End;
                        gs.hint = Some("No more points".to_string());
                        io.of("/xplanet")
                            .unwrap()
                            .to(room_id.clone())
                            .emit("game_state", &gs)
                            .await
                            .ok();
                        continue;
                    };
                    gs.start_index = second_point.index;
                    gs.end_index = second_point.index + gs.map_type.sector_count() / 2 - 1;
                    if gs.end_index > gs.map_type.sector_count() {
                        gs.end_index -= gs.map_type.sector_count();
                    }
                    broadcast_room_game_state(&io, gs).await;
                    broadcast_room_board_token(&io, &gs.id, ss).await;
                }

                // each users should publish tokens
                // check publish first then proposal, we could update tokens after proposal
                if gs.status == GameState::AutoMove && gs.game_stage == GameStage::MeetingPublish {
                    let mut user_points =
                        gs.users.iter().map(Into::into).collect::<Vec<PointInfo>>();
                    user_points.sort_by(|a, b| {
                        a.index
                            .cmp(&b.index)
                            .then_with(|| a.child_index.cmp(&b.child_index))
                    });
                    info!(?user_points, "user points");

                    let mut need_publish = false;
                    for id in user_points.iter().filter_map(|p| {
                        if let PointType::User(id) = &p.r#type {
                            Some(id.clone())
                        } else {
                            None
                        }
                    }) {
                        if ss
                            .user_tokens
                            .get(&id)
                            .is_some_and(|tokens| tokens.iter().any(|t| t.any_ready_published()))
                        {
                            gs.status = GameState::Wait(vec![id.clone()]);
                            let name = gs
                                .users
                                .iter()
                                .find(|u| u.id == id)
                                .map(|u| u.name.clone())
                                .unwrap_or_else(|| "Unknown".to_string());
                            gs.hint = Some(format!("{} should publish", name));
                            need_publish = true;
                            break;
                        }
                    }

                    if !need_publish {
                        // push tokens forword at any none revealed sector
                        // first we need to get revealed sector index
                        let revealed_sectors = ss
                            .user_tokens
                            .iter()
                            .flat_map(|(_user_id, tokens)| {
                                tokens.iter().filter_map(|t| {
                                    t.is_revealed_checked().then_some(t.secret.sector_index)
                                })
                            })
                            .collect::<Vec<_>>();

                        ss.user_tokens.iter_mut().for_each(|(_user_id, tokens)| {
                            tokens
                                .iter_mut()
                                .for_each(|t| t.push_at_meeting(&revealed_sectors));
                        });

                        // check if need to go to meeting check phase
                        if ss
                            .user_tokens
                            .iter()
                            .any(|(_user_id, tokens)| tokens.iter().any(|t| t.any_ready_checked()))
                        {
                            gs.status = GameState::AutoMove;
                            gs.game_stage = GameStage::MeetingCheck;
                            gs.hint = Some(
                                "Push forward triggle Meeting check, Wait Checking...".to_string(),
                            );
                        } else {
                            // no one need to publish, go to next user
                            gs.status = GameState::AutoMove;
                            gs.game_stage = GameStage::UserMove;
                            gs.hint = Some("Push forward".to_string());
                            // need to find next user to move
                            let Some(second_point) = find_next_point(gs, true) else {
                                gs.status = GameState::End;
                                gs.hint = Some("No more points".to_string());
                                io.of("/xplanet")
                                    .unwrap()
                                    .to(room_id.clone())
                                    .emit("game_state", &gs)
                                    .await
                                    .ok();
                                continue;
                            };
                            gs.start_index = second_point.index;
                            gs.end_index = second_point.index + gs.map_type.sector_count() / 2 - 1;
                            if gs.end_index > gs.map_type.sector_count() {
                                gs.end_index -= gs.map_type.sector_count();
                            }
                        }
                    }

                    // make waiting next user move
                    broadcast_room_game_state(&io, gs).await;
                    broadcast_room_board_token(&io, &gs.id, ss).await;
                }

                // proposal finished, and waiting for each user publish
                if gs.status == GameState::AutoMove && gs.game_stage == GameStage::MeetingProposal {
                    info!("server MeetingPublish");
                    gs.game_stage = GameStage::MeetingPublish;
                    gs.hint = Some("Gathering all tokens, ready for Meeting publish".to_string());
                    broadcast_room_game_state(&io, gs).await;
                    broadcast_room_board_token(&io, &gs.id, ss).await;
                    updated_tokens.push(ss.user_tokens.clone());
                }

                if gs.status == GameState::AutoMove && gs.game_stage == GameStage::LastMove {
                    // int the last move, everyone before the winner will have one chance to move
                    // and then the game will end
                    let mut user_points =
                        gs.users.iter().map(Into::into).collect::<Vec<PointInfo>>();
                    user_points.sort_by(|a, b| {
                        a.index
                            .cmp(&b.index)
                            .then_with(|| a.child_index.cmp(&b.child_index))
                    });
                    info!(?user_points, "user points");
                    let mut need_wait_last_move = false;
                    for id in user_points.iter().filter_map(|p| {
                        if let PointType::User(id) = &p.r#type {
                            Some(id.clone())
                        } else {
                            None
                        }
                    }) {
                        let Some(user) = gs.users.iter_mut().find(|u| u.id == id) else {
                            continue;
                        };
                        if !user.last_move {
                            continue;
                        }
                        gs.status = GameState::Wait(vec![id.clone()]);
                        let name = gs
                            .users
                            .iter()
                            .find(|u| u.id == id)
                            .map(|u| u.name.clone())
                            .unwrap_or_else(|| "Unknown".to_string());
                        gs.hint = Some(format!("{} should make last move", name));
                        need_wait_last_move = true;
                        break;
                    }
                    if !need_wait_last_move {
                        // no one need to move, end the game
                        gs.status = GameState::End;
                        gs.game_stage = GameStage::GameEnd;
                        gs.hint = Some("Game Over!".to_string());

                        // reveal all tokens
                        ss.user_tokens.iter_mut().for_each(|(_user_id, tokens)| {
                            tokens.iter_mut().for_each(|t| {
                                if t.reveal_in_the_end()
                                    && !ss.map.meeting_check(t.secret.sector_index, &t.r#type)
                                {
                                    // wrong, move to 4
                                    t.secret.meeting_index = 4;
                                }
                            });
                        });
                    }

                    broadcast_room_game_state(&io, gs).await;
                    broadcast_room_board_token(&io, &gs.id, ss).await;
                }
            }
            for tokens in &updated_tokens {
                send_each_token(&state, tokens);
            }
        }
    });
}

fn find_next_point(gs: &mut GameStateResp, next_next: bool) -> Option<PointInfo> {
    let index = if next_next { 1 } else { 0 };
    let mut all_points: Vec<PointInfo> = gs
        .users
        .iter()
        .map(Into::into)
        .chain(
            gs.map_type
                .meeting_points()
                .into_iter()
                .map(|(index, child_index)| PointInfo {
                    r#type: PointType::Meeting,
                    index,
                    child_index,
                }),
        )
        .chain(
            gs.map_type
                .xclue_points()
                .into_iter()
                .map(|(index, child_index)| PointInfo {
                    r#type: PointType::XClue,
                    index,
                    child_index,
                }),
        )
        .collect::<Vec<_>>();
    // sort by start_index, index, child_index
    all_points.sort_by(|a, b| {
        a.index
            .cmp(&b.index)
            .then_with(|| a.child_index.cmp(&b.child_index))
    });
    info!(?all_points, "all points");

    all_points
        .iter()
        .cycle()
        .skip_while(|p| p.index < gs.start_index)
        .nth(index)
        .cloned()
}

async fn broadcast_room_game_state(io: &SocketIo, gs: &mut GameStateResp) {
    // let mut gs = gs.clone();
    // gs.users.iter_mut().for_each(|u| {
    //     u.moves_result.clear();
    // });

    io.of("/xplanet")
        .unwrap()
        .to(gs.id.clone())
        .emit("game_state", &gs)
        .await
        .ok();
}

async fn broadcast_room_board_token(io: &SocketIo, room_id: &str, ss: &ServerGameState) {
    let tokens = ss
        .user_tokens
        .iter()
        .flat_map(|(_user_id, tokens)| tokens.iter())
        .filter(|t| t.placed)
        .map(|t| &t.secret)
        .cloned()
        .collect::<Vec<_>>();
    io.of("/xplanet")
        .unwrap()
        .to(room_id.to_owned())
        .emit("board_tokens", &tokens)
        .await
        .ok();
}

fn send_each_token(
    state: &tokio::sync::MutexGuard<'_, crate::server_state::State>,
    tokens: &HashMap<String, Vec<crate::map::Token>>,
) {
    for (user_id, token) in tokens {
        let s = state
            .users
            .iter()
            .find_map(|(_sid, (s, u))| (u.id == *user_id).then_some(s.clone()));
        let Some(user_socket) = s else {
            tracing::error!("user not found, user_id: {}", user_id);
            continue;
        };
        user_socket.emit("token", token).ok();
    }
}

#[derive(Debug, Clone)]
struct PointInfo {
    r#type: PointType,
    index: usize,
    child_index: usize,
}

#[derive(Debug, Clone)]
enum PointType {
    User(String),
    Meeting,
    XClue,
}

impl From<&UserState> for PointInfo {
    fn from(user: &UserState) -> Self {
        PointInfo {
            r#type: PointType::User(user.id.clone()),
            index: user.location.index,
            child_index: user.location.child_index,
        }
    }
}
