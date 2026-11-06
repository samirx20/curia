//! One HTTPS POST to a push service (Google's for Chrome, Apple's for Safari). A request this
//! small doesn't need an HTTP client library: rustls with the system's trusted certificates,
//! one request, read the status line.
use anyhow::{Context, Result, bail};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

fn client() -> Result<Arc<rustls::ClientConfig>> {
    static CONFIG: OnceLock<Arc<rustls::ClientConfig>> = OnceLock::new();
    if let Some(c) = CONFIG.get() {
        return Ok(c.clone());
    }
    let mut roots = rustls::RootCertStore::empty();
    for cert in rustls_native_certs::load_native_certs().certs {
        let _ = roots.add(cert);
    }
    if roots.is_empty() {
        bail!("no trusted certificates found on this machine");
    }
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()?
        .with_root_certificates(roots)
        .with_no_client_auth();
    Ok(CONFIG.get_or_init(|| Arc::new(config)).clone())
}

/// `https://host[:port]/path` → (host, port, path, origin).
pub fn parse(endpoint: &str) -> Result<(String, u16, String, String)> {
    let rest = endpoint.strip_prefix("https://").context("a push endpoint must be https")?;
    let (authority, path) = rest.split_once('/').map(|(a, p)| (a, format!("/{p}"))).unwrap_or((rest, "/".into()));
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => (h.to_string(), p.parse().context("bad port in the push endpoint")?),
        None => (authority.to_string(), 443),
    };
    let origin = if port == 443 { format!("https://{host}") } else { format!("https://{host}:{port}") };
    Ok((host, port, path, origin))
}

/// Posts an encrypted message; answers with the HTTP status (201 means delivered).
pub async fn post(endpoint: &str, authorization: &str, body: &[u8]) -> Result<u16> {
    let (host, port, path, _) = parse(endpoint)?;
    let work = async {
        let tcp = TcpStream::connect((host.as_str(), port)).await.with_context(|| format!("could not reach {host}"))?;
        let name = rustls_pki_types::ServerName::try_from(host.clone()).context("bad push service name")?;
        let mut tls = TlsConnector::from(client()?).connect(name, tcp).await.context("TLS to the push service failed")?;
        let head = format!(
            "POST {path} HTTP/1.1\r\nHost: {host}\r\nAuthorization: {authorization}\r\nTTL: 86400\r\nUrgency: high\r\n\
             Content-Encoding: aes128gcm\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        tls.write_all(head.as_bytes()).await?;
        tls.write_all(body).await?;
        tls.flush().await?;
        let mut reply = vec![0u8; 512];
        let n = tls.read(&mut reply).await?;
        let line = String::from_utf8_lossy(&reply[..n]);
        let status = line.split_whitespace().nth(1).and_then(|s| s.parse().ok()).with_context(|| format!("odd reply from the push service: {line:.80}"))?;
        anyhow::Ok(status)
    };
    tokio::time::timeout(Duration::from_secs(20), work).await.context("the push service didn't answer within 20 s")?
}

#[cfg(test)]
mod tests {
    #[test]
    fn endpoints_split_into_host_path_and_origin() {
        let (h, p, path, origin) = super::parse("https://fcm.googleapis.com/fcm/send/abc:def").unwrap();
        assert_eq!((h.as_str(), p, path.as_str(), origin.as_str()), ("fcm.googleapis.com", 443, "/fcm/send/abc:def", "https://fcm.googleapis.com"));
        assert!(super::parse("http://example.com/x").is_err());
    }
}
