use axum::response::Response;
use axum::{body::Body, http::StatusCode};

pub async fn fallback_handler() -> Response {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("Not found"))
        .unwrap()
}
