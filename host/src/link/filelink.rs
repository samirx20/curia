//! `GET /file/<token>`: the bytes behind a link from `file_link` (images, video, downloads).
//! Byte ranges are honoured, so a video can seek; a committed version (image compare) is
//! sent whole.
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use std::io::SeekFrom;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

pub async fn serve(State((host, _)): State<super::AppState>, Path(token): Path<String>, headers: HeaderMap) -> Response {
    let Some(grant) = host.file_grant(&token) else {
        return (StatusCode::NOT_FOUND, "This file link has expired; open the file again.").into_response();
    };
    let kind = kind(&grant.path);
    let name = grant.path.rsplit('/').next().unwrap_or("file").replace('"', "");
    let disposition = format!("inline; filename=\"{name}\"");
    if grant.rev.is_some() {
        return match tokio::task::spawn_blocking(move || crate::host::committed(&grant)).await {
            Ok(Ok(bytes)) => ([(header::CONTENT_TYPE, kind.to_string()), (header::CONTENT_DISPOSITION, disposition)], Body::from(bytes)).into_response(),
            Ok(Err(e)) => (StatusCode::NOT_FOUND, format!("{e:#}")).into_response(),
            Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        };
    }
    let full = grant.folder.join(&grant.path);
    let Ok(mut file) = tokio::fs::File::open(&full).await else {
        return (StatusCode::NOT_FOUND, "The file is gone.").into_response();
    };
    let Ok(size) = file.metadata().await.map(|m| m.len()) else { return StatusCode::INTERNAL_SERVER_ERROR.into_response() };
    let range = headers.get(header::RANGE).and_then(|v| v.to_str().ok()).and_then(|r| parse_range(r, size));
    let (start, end) = range.unwrap_or((0, size.saturating_sub(1)));
    let len = if size == 0 { 0 } else { end - start + 1 };
    if file.seek(SeekFrom::Start(start)).await.is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    let body = Body::from_stream(tokio_util_stream(file.take(len)));
    let mut res = Response::builder()
        .header(header::CONTENT_TYPE, kind)
        .header(header::CONTENT_DISPOSITION, disposition)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_LENGTH, len);
    res = if range.is_some() {
        res.status(StatusCode::PARTIAL_CONTENT).header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{size}"))
    } else {
        res.status(StatusCode::OK)
    };
    res.body(body).unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

/// Streams a reader in 64 KB pieces.
fn tokio_util_stream<R: tokio::io::AsyncRead + Unpin + Send + 'static>(reader: R) -> impl futures_util::Stream<Item = std::io::Result<Vec<u8>>> + Send {
    futures_util::stream::unfold(reader, |mut r| async move {
        let mut buf = vec![0u8; 64 * 1024];
        match r.read(&mut buf).await {
            Ok(0) => None,
            Ok(n) => {
                buf.truncate(n);
                Some((Ok(buf), r))
            }
            Err(e) => Some((Err(e), r)),
        }
    })
}

/// `bytes=a-b`, `bytes=a-` or `bytes=-n`, within `size`.
fn parse_range(header: &str, size: u64) -> Option<(u64, u64)> {
    let spec = header.strip_prefix("bytes=")?.split(',').next()?.trim();
    let (a, b) = spec.split_once('-')?;
    let last = size.checked_sub(1)?;
    let (start, end) = match (a.trim(), b.trim()) {
        ("", n) => (size.saturating_sub(n.parse().ok()?), last),
        (s, "") => (s.parse().ok()?, last),
        (s, e) => (s.parse().ok()?, e.parse::<u64>().ok()?.min(last)),
    };
    (start <= end).then_some((start, end))
}

fn kind(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("").to_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "pdf" => "application/pdf",
        "md" | "txt" | "rs" | "ts" | "js" | "json" | "toml" | "py" | "qml" | "html" | "css" | "yml" | "yaml" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::parse_range;

    #[test]
    fn ranges_are_read_and_kept_inside_the_file() {
        assert_eq!(parse_range("bytes=0-99", 1000), Some((0, 99)));
        assert_eq!(parse_range("bytes=900-", 1000), Some((900, 999)));
        assert_eq!(parse_range("bytes=-100", 1000), Some((900, 999)));
        assert_eq!(parse_range("bytes=990-5000", 1000), Some((990, 999)));
        assert_eq!(parse_range("bytes=50-10", 1000), None);
        assert_eq!(parse_range("bytes=0-1", 0), None);
    }
}
