use axum::{
    Router,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use serde::Deserialize;
use std::sync::Arc;
use tokio::fs::File;

#[derive(Deserialize)]
pub struct DownloadQuery {
    #[allow(dead_code)]
    pub task_id: String,
}

pub struct AppState {
    pub file_path: String,
}

pub async fn start_http_server(port: u16, file_path: String) -> anyhow::Result<()> {
    let state = Arc::new(AppState { file_path });

    let app = Router::new()
        .route("/download", get(handle_download))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    println!("HTTP Server listening on port {}", port);
    axum::serve(listener, app).await?;

    Ok(())
}

async fn handle_download(
    Query(_query): Query<DownloadQuery>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    match File::open(&state.file_path).await {
        Ok(file) => {
            let stream = tokio_util::io::ReaderStream::new(file);
            (StatusCode::OK, axum::body::Body::from_stream(stream)).into_response()
        }
        Err(_) => (StatusCode::NOT_FOUND, "File not found").into_response(),
    }
}
