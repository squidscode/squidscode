use axum::{
    extract::{Path, Request, State},
    http::StatusCode,
    response::{Html, Json, Redirect},
    routing::{any, get},
    Router,
};
use serde::Serialize;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

#[derive(Clone, Serialize)]
struct ReceivedRequest {
    method: String,
    headers: Vec<(String, String)>,
    body: String,
    received_at: u64,
}

pub struct WebhookEntry {
    requests: Vec<ReceivedRequest>,
}

pub type WebhookStore = Arc<Mutex<HashMap<String, WebhookEntry>>>;

pub fn new_store() -> WebhookStore {
    Arc::new(Mutex::new(HashMap::new()))
}

async fn webhooks_get() -> Html<&'static str> {
    Html(include_str!("../static/webhooks.html"))
}

async fn webhooks_post(State(store): State<WebhookStore>) -> Redirect {
    let id = Uuid::new_v4().simple().to_string();
    store.lock().unwrap().insert(id.clone(), WebhookEntry { requests: vec![] });
    Redirect::to(&format!("/webhooks/{id}"))
}

async fn view_webhook(
    State(store): State<WebhookStore>,
    Path(id): Path<String>,
) -> Result<Html<String>, StatusCode> {
    store.lock().unwrap().contains_key(&id).then_some(()).ok_or(StatusCode::NOT_FOUND)?;
    let html = include_str!("../static/webhooks-view.html").replace("{id}", &id);
    Ok(Html(html))
}

async fn list_requests(
    State(store): State<WebhookStore>,
    Path(id): Path<String>,
) -> Result<Json<Vec<ReceivedRequest>>, StatusCode> {
    let store = store.lock().unwrap();
    let entry = store.get(&id).ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(entry.requests.clone()))
}

async fn receive(
    State(store): State<WebhookStore>,
    Path(id): Path<String>,
    req: Request,
) -> StatusCode {
    let method = req.method().to_string();
    let headers = req
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();
    let body_bytes = axum::body::to_bytes(req.into_body(), 1024 * 1024)
        .await
        .unwrap_or_default();
    let body = String::from_utf8_lossy(&body_bytes).into_owned();
    let received_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut store = store.lock().unwrap();
    match store.get_mut(&id) {
        Some(entry) => {
            entry.requests.push(ReceivedRequest { method, headers, body, received_at });
            StatusCode::OK
        }
        None => StatusCode::NOT_FOUND,
    }
}

pub fn router(store: WebhookStore) -> Router {
    Router::new()
        .route("/webhooks", get(webhooks_get).post(webhooks_post))
        .route("/webhooks/:id", get(view_webhook))
        .route("/webhooks/:id/requests", get(list_requests))
        .route("/webhooks/:id/receive", any(receive))
        .with_state(store)
}
