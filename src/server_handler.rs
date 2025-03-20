use crate::{
    operation::Operation,
    room::RoomUserOperation,
    server_state::{StateRef, User},
};
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

    match state.lock().await.handle_action_op(user, op) {
        Ok(resp) => {
            // to the user
            info!(ns = "socket.io", ?socket.id, ?resp, "op success");
            socket.emit("op_result", &resp).ok();
            // to other users in the room
            // todo
        }
        Err(e) => {
            info!(ns = "socket.io", ?socket.id, ?e, "op error");
            socket.emit("server_resp", &format!("op error {e}")).ok();
        }
    }
}

async fn handle_room(io: SocketIo, socket: SocketRef, state: StateRef, op: RoomUserOperation) {
    let user = state.lock().await.check_auth(socket.id.as_str()).cloned();
    let Some(user) = user else {
        info!(ns = "socket.io", ?socket.id, "unauthorized room op {:?}", op);
        return;
    };

    info!(?op, ?socket.id, "received room op {:?}", op);

    match state.lock().await.handle_room_op(user, op) {
        Ok(resp) => {
            for r in resp {
                info!(ns = "socket.io", ?socket.id, ?r, "room op success");
                // to every user in the room
                io.of("/xplanet")
                    .unwrap()
                    .to(r.room_id.clone())
                    .emit("room_result", &r)
                    .await
                    .ok();
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
