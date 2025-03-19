use crate::{
    operation::Operation,
    room::RoomUserOperation,
    server_state::{StateRef, User},
};
use socketioxide::extract::{Data, SocketRef, State};
use tracing::info;

pub async fn handle_on_connect(socket: SocketRef, _state: State<StateRef>) {
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
        |socket: SocketRef, State::<StateRef>(state), Data::<Operation>(op)| async move {
            handle_op(socket, state, op).await;
        },
    );

    socket.on(
        "room",
        |socket: SocketRef, State::<StateRef>(state), Data::<RoomUserOperation>(op)| async move {
            handle_room(socket, state, op).await;
        },
    );
}

async fn handle_op(socket: SocketRef, state: StateRef, op: Operation) {
    if state.lock().await.check_auth(socket.id.as_str()).is_none() {
        info!(ns = "socket.io", ?socket.id, "unauthorized op {:?}", op);
        return;
    }
    info!(?op, ?socket.id, "received op {:?}", op);
}

async fn handle_room(socket: SocketRef, state: StateRef, op: RoomUserOperation) {
    if state.lock().await.check_auth(socket.id.as_str()).is_none() {
        info!(ns = "socket.io", ?socket.id, "unauthorized op {:?}", op);
        return;
    }
    info!(?op, ?socket.id, "received op {:?}", op);
}
