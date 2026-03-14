use std::{collections::HashMap, net::SocketAddr, sync::{Arc, Mutex}};
use tower_http::services::ServeDir;
use axum::Router;

mod pastebin;
mod files;
mod webhooks;

#[tokio::main]
async fn main() {
    let paste_store = Arc::new(Mutex::new(HashMap::new()));
    let file_store = files::new_store();
    let webhook_store = webhooks::new_store();

    let app = Router::new()
        .merge(pastebin::router(paste_store))
        .merge(files::router(file_store))
        .merge(webhooks::router(webhook_store))
        .fallback_service(ServeDir::new("static"));

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("Serving at http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
