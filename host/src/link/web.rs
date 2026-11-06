//! Serves the phone web app (Tech Stack §5): the built files from `web/dist`, beside the
//! program when installed, the source folder while developing. Paths can't leave that folder;
//! any other path gets `index.html`, so the app's own routes load it.
use axum::body::Body;
use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use std::path::{Component, Path, PathBuf};

fn root() -> PathBuf {
    let beside = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("web")));
    beside.filter(|d| d.join("index.html").is_file())
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("web").join("dist"))
}

pub async fn file(uri: Uri) -> Response {
    let root = root();
    let wanted = uri.path().trim_start_matches('/');
    // Only plain names: no `..`, no drive letters, no root.
    let safe = Path::new(wanted).components().all(|c| matches!(c, Component::Normal(_)));
    let path = if safe && !wanted.is_empty() && root.join(wanted).is_file() { root.join(wanted) } else { root.join("index.html") };
    match tokio::fs::read(&path).await {
        Ok(bytes) => {
            let cache = if path.ends_with("index.html") || path.ends_with("sw.js") { "no-cache" } else { "public, max-age=31536000, immutable" };
            ([(header::CONTENT_TYPE, kind(&path)), (header::CACHE_CONTROL, cache)], Body::from(bytes)).into_response()
        }
        Err(_) => (StatusCode::NOT_FOUND, "The web app isn't built yet: run `npm run build` in web/.").into_response(),
    }
}

fn kind(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "webmanifest" => "application/manifest+json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn paths_outside_the_app_never_serve_the_file() {
        for (bad, inside) in [("/../Cargo.toml", "[package]"), ("/..%2f..%2fCargo.toml", "[package]"), ("/C:/Windows/win.ini", "[fonts]")] {
            let res = file(bad.parse().unwrap()).await;
            let body = axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap();
            assert!(!String::from_utf8_lossy(&body).contains(inside), "{bad} served the file");
        }
    }
}
