use crate::{
    operation::Operation,
    room::{GameState, GameStateResp, RoomUserOperation, ServerGameState, UserLocationSequence},
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
                .upsert_user(user.0.clone(), socket.id.to_string());
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
            socket.to("room_id").emit("op", &op).await.ok();
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
                if gs.users.iter().find(|&u| &u.id == &user.id).is_some() {
                    socket.emit("game_state", &gs).ok();
                    do_resp = true;
                }

                // // to every user in the room
                // io.of("/xplanet")
                //     .unwrap()
                //     .to(gs.id.clone())
                //     .emit("game_state", &gs)
                //     .await
                //     .ok();
            }
            if !do_resp {
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
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(2));
    tokio::task::spawn(async move {
        loop {
            interval.tick().await;
            let mut state = state.lock().await;

            // 1. clean empty game rooms
            let mut clean_room_ids = Vec::new();
            for (room_id, gs) in state.game_state.iter() {
                if gs.users.is_empty() {
                    clean_room_ids.push(room_id.clone());
                }
            }
            for room_id in clean_room_ids {
                state.game_state.remove(&room_id);
                state.map_data.remove(&room_id);
            }

            // 2.1 check if all users in a room are ready, and start the game
            let mut game_start_data = Vec::new();
            for (room_id, gs) in state.game_state.iter_mut() {
                if gs.status == GameState::NotStarted && gs.users.iter().all(|u| u.ready) {
                    gs.status = GameState::Starting;
                    gs.hint = Some("Game is starting".to_string());
                    io.of("/xplanet")
                        .unwrap()
                        .to(gs.id.clone())
                        .emit("game_state", &gs)
                        .await
                        .ok();
                    // todo start game generate map
                    gs.start_index = 1;
                    gs.end_index = gs.map_type.sector_count() / 2;
                    gs.users.shuffle(&mut SmallRng::seed_from_u64(gs.map_seed));
                    for (index, user) in gs.users.iter_mut().enumerate() {
                        user.should_move = index == 0;
                        user.location = UserLocationSequence::new(gs.start_index, index + 1);
                    }

                    io.of("/xplanet")
                        .unwrap()
                        .to(gs.id.clone())
                        .emit("game_state", &gs)
                        .await
                        .ok();

                    let rng = SmallRng::seed_from_u64(gs.map_seed);
                    let Ok(map) = crate::map::Map::new(rng, gs.map_type.clone()) else {
                        gs.status = GameState::End;
                        gs.hint = Some("Map generation failed".to_string());
                        io.of("/xplanet")
                            .unwrap()
                            .to(gs.id.clone())
                            .emit("game_state", &gs)
                            .await
                            .ok();
                        continue;
                    };
                    let Ok((research_clues, x_clues)) =
                        crate::map::ClueGenerator::new(gs.map_seed, map.sectors.clone())
                            .generate_clues()
                    else {
                        gs.status = GameState::End;
                        gs.hint = Some("Clue generation failed".to_string());
                        io.of("/xplanet")
                            .unwrap()
                            .to(gs.id.clone())
                            .emit("game_state", &gs)
                            .await
                            .ok();
                        continue;
                    };
                    game_start_data.push((
                        room_id.clone(),
                        ServerGameState {
                            map,
                            research_clues,
                            x_clues,
                        },
                    ));
                }
            }
            // 2.2 send game start data, update map data
            for (room_id, server_game_state) in game_start_data {
                io.of("/xplanet")
                    .unwrap()
                    .to(room_id.clone())
                    .emit("game_start", &server_game_state.clue_secret())
                    .await
                    .ok();
                state.map_data.insert(room_id.clone(), server_game_state);
                let Some(gs) = state.game_state.get_mut(&room_id) else {
                    tracing::error!("game state not found, room_id: {}", room_id);
                    continue;
                };
                gs.status = GameState::Wait(gs.users[0].id.clone());
                gs.hint = Some("Game started".to_string());
                io.of("/xplanet")
                    .unwrap()
                    .to(room_id.clone())
                    .emit("game_state", &gs)
                    .await
                    .ok();
            }
        }
    });
}
