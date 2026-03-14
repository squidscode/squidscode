use axum::{
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::{header, StatusCode},
    response::{Html, Redirect, Response},
    routing::get,
    Router,
};
use serde::Deserialize;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::time::{sleep, Duration};
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

pub struct FileEntry {
    filename: String,
    content_type: String,
    data: Vec<u8>,
}

pub type FileStore = Arc<Mutex<HashMap<String, FileEntry>>>;

pub fn new_store() -> FileStore {
    Arc::new(Mutex::new(HashMap::new()))
}

fn format_size(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

async fn files_get() -> Html<&'static str> {
    Html(include_str!("../static/files.html"))
}

async fn files_post(
    State(store): State<FileStore>,
    mut multipart: Multipart,
) -> Result<Redirect, StatusCode> {
    while let Some(field) = multipart.next_field().await.map_err(|_| StatusCode::BAD_REQUEST)? {
        if field.name() != Some("file") {
            continue;
        }
        let filename = field
            .file_name()
            .unwrap_or("upload")
            .to_string();
        let content_type = field
            .content_type()
            .unwrap_or("application/octet-stream")
            .to_string();
        let data = field.bytes().await.map_err(|_| StatusCode::BAD_REQUEST)?.to_vec();

        let id = Uuid::new_v4().simple().to_string();
        store.lock().unwrap().insert(id.clone(), FileEntry { filename, content_type, data });
        return Ok(Redirect::to(&format!("/files/{id}")));
    }
    Err(StatusCode::BAD_REQUEST)
}

async fn view_file(
    State(store): State<FileStore>,
    Path(id): Path<String>,
) -> Result<Html<String>, StatusCode> {
    let store = store.lock().unwrap();
    let entry = store.get(&id).ok_or(StatusCode::NOT_FOUND)?;
    let html = include_str!("../static/files-view.html")
        .replace("{filename}", &entry.filename)
        .replace("{content_type}", &entry.content_type)
        .replace("{size}", &format_size(entry.data.len()))
        .replace("{id}", &id);
    Ok(Html(html))
}

#[derive(Deserialize)]
struct DownloadParams {
    /// Rate limit in KB/s.
    rate: Option<f64>,
    /// Total download duration in seconds (derives rate from file size).
    duration: Option<f64>,
}

async fn download_file(
    State(store): State<FileStore>,
    Path(id): Path<String>,
    Query(params): Query<DownloadParams>,
) -> Result<Response<Body>, StatusCode> {
    let store = store.lock().unwrap();
    let entry = store.get(&id).ok_or(StatusCode::NOT_FOUND)?;
    let disposition = format!("attachment; filename=\"{}\"", entry.filename);
    let data = entry.data.clone();
    let content_type = entry.content_type.clone();
    drop(store);

    if params.rate.is_some() && params.duration.is_some() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let rate = params.rate.or_else(|| {
        params.duration.filter(|&d| d > 0.0).map(|d| data.len() as f64 / (d * 1024.0))
    });

    let body = match rate.filter(|&r| r > 0.0) {
        None => Body::from(data),
        Some(kbps) => {
            // Send in ~4 KB chunks, sleeping between each to hit the target rate.
            const CHUNK: usize = 4 * 1024;
            let delay = Duration::from_secs_f64(CHUNK as f64 / (kbps * 1024.0));
            let (tx, rx) = tokio::sync::mpsc::channel::<Result<Vec<u8>, std::io::Error>>(1);
            tokio::spawn(async move {
                for chunk in data.chunks(CHUNK) {
                    if tx.send(Ok(chunk.to_vec())).await.is_err() {
                        break;
                    }
                    sleep(delay).await;
                }
            });
            Body::from_stream(ReceiverStream::new(rx))
        }
    };

    let response = Response::builder()
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_DISPOSITION, disposition)
        .body(body)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(response)
}

pub fn router(store: FileStore) -> Router {
    Router::new()
        .route("/files", get(files_get).post(files_post))
        .route("/files/:id", get(view_file))
        .route("/files/:id/raw", get(download_file))
        .with_state(store)
}
