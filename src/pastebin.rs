use axum::{
    extract::{Form, Path, State},
    http::StatusCode,
    response::{Html, Redirect},
    routing::get,
    Router,
};
use serde::Deserialize;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

pub type PasteStore = Arc<Mutex<HashMap<String, String>>>;

#[derive(Deserialize)]
struct PasteForm {
    content: String,
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

async fn pastebin_get() -> Html<&'static str> {
    Html(include_str!("../static/pastebin.html"))
}

async fn pastebin_post(
    State(store): State<PasteStore>,
    Form(form): Form<PasteForm>,
) -> Redirect {
    let id = Uuid::new_v4().simple().to_string();
    store.lock().unwrap().insert(id.clone(), form.content);
    Redirect::to(&format!("/pastebin/{id}"))
}

async fn view_paste(
    State(store): State<PasteStore>,
    Path(id): Path<String>,
) -> Result<Html<String>, StatusCode> {
    let store = store.lock().unwrap();
    let content = store.get(&id).ok_or(StatusCode::NOT_FOUND)?;
    let escaped = html_escape(content);
    let html = include_str!("../static/pastebin-view.html")
        .replace("{id}", &id)
        .replace("{escaped}", &escaped);
    Ok(Html(html))
}

pub fn router(store: PasteStore) -> Router {
    Router::new()
        .route("/pastebin", get(pastebin_get).post(pastebin_post))
        .route("/pastebin/:id", get(view_paste))
        .with_state(store)
}
