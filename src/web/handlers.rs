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
    "OpenDLNA Media Server"
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
        let range_str = range_header.to_str().map_err(|_| AppError::InvalidRange)?;
        debug!("Received range request: {}", range_str);
        
        // Parse the range header manually to avoid enum variant issues
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

// Helper function to parse range header manually
fn parse_range_header(range_str: &str, file_size: u64) -> Result<(u64, u64), AppError> {
    // Remove "bytes=" prefix
    let range_part = range_str.strip_prefix("bytes=").ok_or(AppError::InvalidRange)?;
    
    // Split on comma to get individual ranges (we'll just handle the first one)
    let first_range = range_part.split(',').next().ok_or(AppError::InvalidRange)?;
    
    // Parse the range
    if let Some((start_str, end_str)) = first_range.split_once('-') {
        let start = if start_str.is_empty() {
            // Suffix range like "-500" (last 500 bytes)
            let suffix_len: u64 = end_str.parse().map_err(|_| AppError::InvalidRange)?;
            if suffix_len >= file_size {
                0
            } else {
                file_size - suffix_len
            }
        } else {
            start_str.parse().map_err(|_| AppError::InvalidRange)?
        };
        
        let end = if end_str.is_empty() {
            // Range like "500-" (from 500 to end)
            file_size - 1
        } else {
            let parsed_end: u64 = end_str.parse().map_err(|_| AppError::InvalidRange)?;
            parsed_end.min(file_size - 1)
        };
        
        // Validate range
        if start > end || start >= file_size {
            return Err(AppError::InvalidRange);
        }
        
        Ok((start, end))
    } else {
        Err(AppError::InvalidRange)
    }
}