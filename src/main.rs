use salvo::{Listener, Router, Server, conn::TcpListener, handler, prelude::TowerLayerCompat};
use serde::{Deserialize, Serialize};
use socketioxide::{
    SocketIo,
    extract::{Data, SocketRef},
};
use tracing::info;
use tracing_subscriber::FmtSubscriber;

#[handler]
async fn hello() -> &'static str {
    "Hello Salvo!"
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let subscriber = FmtSubscriber::new();
    tracing::subscriber::set_global_default(subscriber)?;

    let (layer, io) = SocketIo::new_layer();

    let layer = tower::ServiceBuilder::new()
        .layer(tower_http::cors::CorsLayer::permissive())
        .layer(layer);

    io.ns("/", on_connect);
    io.ns("/custom", on_connect);

    let layer = layer.compat();
    let router = Router::with_path("/socket.io").hoop(layer).goal(hello);
    let acceptor = TcpListener::new("127.0.0.1:7878").bind().await;
    Server::new(acceptor).serve(router).await;

    Ok(())
}

fn on_connect(socket: SocketRef) {
    // info!(ns = "socket.io", "connected: {:?}", data);
    info!(ns = "socket.io", "connected");
    // socket.emit("auth", &data).ok();

    socket.on("message", |socket: SocketRef, Data::<Message>(data)| {
        info!(?data, "received message");
        socket.emit("message-back", &data).ok();
    });
}

#[derive(Debug, Serialize, Deserialize)]
struct Message {
    message: String,
}
