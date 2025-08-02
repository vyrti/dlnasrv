use crate::{
    error::AppError,
    state::AppState,
    web::xml::{generate_browse_response, generate_description_xml, generate_scpd_xml},
};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use futures_util::StreamExt;
use tokio::fs::File;
use tokio::io::AsyncSeekExt;
use tokio_util::io::ReaderStream;
use tracing::debug;

pub async fn root_handler() -> &'static str {
    "Rust DLNA Media Server"
}

pub async fn description_handler(State(state): State<AppState>) -> impl IntoResponse {
    let xml = generate_description_xml(&state.config);
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/xml; charset=utf-8")],
        xml,
    )
}

pub async fn content_directory_scpd() -> impl IntoResponse {
    let xml = generate_scpd_xml();
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/xml; charset=utf-8")],
        xml,
    )
}

pub async fn content_directory_control(
    State(state): State<AppState>,
    body: String,
) -> Response {
    // A proper implementation would parse the SOAP request.
    // For now, we just check for the Browse action.
    if body.contains("<u:Browse") {
        let media_files = state.media_files.read().await;
        let response = generate_browse_response(&media_files, &state.config);
        (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "text/xml; charset=utf-8"),
                (header::HeaderName::from_static("ext"), ""),
            ],
            response,
        )
            .into_response()
    } else {
        (
            StatusCode::NOT_IMPLEMENTED,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "Not implemented".to_string(),
        )
            .into_response()
    }
}

fn parse_range_header(range_str: &str, file_size: u64) -> Result<(u64, u64), AppError> {
    // Parse range header like "bytes=0-1023" or "bytes=0-" or "bytes=-1024"
    if !range_str.starts_with("bytes=") {
        return Err(AppError::InvalidRange);
    }
    
    let range_part = &range_str[6..]; // Remove "bytes="
    let parts: Vec<&str> = range_part.split('-').collect();
    
    if parts.len() != 2 {
        return Err(AppError::InvalidRange);
    }
    
    let start_str = parts[0];
    let end_str = parts[1];
    
    let (start, end) = if start_str.is_empty() {
        // Suffix range: bytes=-1024 (last 1024 bytes)
        if let Ok(suffix_length) = end_str.parse::<u64>() {
            let start = file_size.saturating_sub(suffix_length);
            (start, file_size - 1)
        } else {
            return Err(AppError::InvalidRange);
        }
    } else if end_str.is_empty() {
        // Range from start to end: bytes=1024-
        if let Ok(start) = start_str.parse::<u64>() {
            if start >= file_size {
                return Err(AppError::InvalidRange);
            }
            (start, file_size - 1)
        } else {
            return Err(AppError::InvalidRange);
        }
    } else {
        // Full range: bytes=0-1023
        let start = start_str.parse::<u64>().map_err(|_| AppError::InvalidRange)?;
        let end = end_str.parse::<u64>().map_err(|_| AppError::InvalidRange)?;
        
        if start > end || start >= file_size {
            return Err(AppError::InvalidRange);
        }
        
        // Clamp end to file size
        let end = std::cmp::min(end, file_size - 1);
        (start, end)
    };
    
    Ok((start, end))
}

pub async fn serve_media(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let media_files = state.media_files.read().await;
    let file_info = media_files
        .iter()
        .find(|f| f.id == id)
        .cloned()
        .ok_or(AppError::NotFound)?;

    let mut file = File::open(&file_info.path).await.map_err(AppError::Io)?;
    let file_size = file_info.size;

    let mut response_builder = Response::builder()
        .header(header::CONTENT_TYPE, file_info.mime_type)
        .header(header::ACCEPT_RANGES, "bytes");

    let (start, end) = if let Some(range_header) = headers.get(header::RANGE) {
        // Handle range requests for streaming
        let range_str = range_header.to_str().map_err(|_| AppError::InvalidRange)?;
        debug!("Received range request: {}", range_str);
        
        parse_range_header(range_str, file_size)?
    } else {
        // No range requested, serve the whole file
        (0, file_size - 1)
    };

    let len = end - start + 1;

    let response_status = if len < file_size {
        response_builder = response_builder.header(
            header::CONTENT_RANGE,
            format!("bytes {}-{}/{}", start, end, file_size),
        );
        StatusCode::PARTIAL_CONTENT
    } else {
        StatusCode::OK
    };

    response_builder = response_builder.header(header::CONTENT_LENGTH, len);

    file.seek(std::io::SeekFrom::Start(start)).await?;
    let stream = ReaderStream::with_capacity(file, 64 * 1024).take(len as usize);
    let body = Body::from_stream(stream);

    Ok(response_builder.status(response_status).body(body)?)
}