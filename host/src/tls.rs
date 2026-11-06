//! HTTPS over Tailscale (Tech Stack §5.2): the host asks `tailscale cert` for a certificate
//! for its own `*.ts.net` name, serves with it, renews it daily and swaps the new one in
//! without restarting. Phones trust these certificates with no setup.
use anyhow::{Context, Result, bail};
use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::CertifiedKey;
use rustls_pki_types::pem::PemObject;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};
use serde_json::Value;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// This machine on the tailnet.
#[derive(Debug, Clone)]
pub struct Tailnet {
    /// Its name with a certificate, e.g. `curia-host.tail1234.ts.net`.
    pub name: String,
    /// Its Tailscale IPv4 address: the only address the remote link listens on.
    pub ip: IpAddr,
}

fn tailscale_exe() -> PathBuf {
    let installed = Path::new(r"C:\Program Files\Tailscale\tailscale.exe");
    if cfg!(windows) && installed.is_file() { installed.to_path_buf() } else { PathBuf::from("tailscale") }
}

/// Reads the name and address from `tailscale status`. Fails loudly if Tailscale isn't
/// running or HTTPS certificates aren't turned on for the tailnet.
pub fn tailnet() -> Result<Tailnet> {
    let out = std::process::Command::new(tailscale_exe()).args(["status", "--json"]).output().context("could not run tailscale; is it installed?")?;
    if !out.status.success() {
        bail!("tailscale status failed: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    let s: Value = serde_json::from_slice(&out.stdout).context("tailscale status gave no JSON")?;
    let name = s["CertDomains"][0].as_str()
        .context("HTTPS certificates aren't turned on for this tailnet (Tailscale admin console → DNS → HTTPS Certificates)")?
        .to_string();
    let ip = s["Self"]["TailscaleIPs"].as_array().into_iter().flatten()
        .filter_map(|v| v.as_str()?.parse::<IpAddr>().ok())
        .find(IpAddr::is_ipv4)
        .context("this machine has no Tailscale address; is Tailscale connected?")?;
    Ok(Tailnet { name, ip })
}

/// Asks Tailscale for a fresh certificate into `dir` (`cert.pem`, `key.pem`).
pub fn fetch(name: &str, dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let (cert, key) = (dir.join("cert.pem"), dir.join("key.pem"));
    let out = std::process::Command::new(tailscale_exe())
        .arg("cert").arg("--cert-file").arg(&cert).arg("--key-file").arg(&key).arg(name)
        .output().context("could not run tailscale cert")?;
    if !out.status.success() {
        bail!("tailscale cert failed: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(())
}

fn load(dir: &Path) -> Result<Arc<CertifiedKey>> {
    let chain = CertificateDer::pem_file_iter(dir.join("cert.pem"))
        .context("could not read the certificate")?
        .collect::<Result<Vec<_>, _>>()
        .context("the certificate file is damaged")?;
    let key = PrivateKeyDer::from_pem_file(dir.join("key.pem")).context("could not read the certificate's key")?;
    let provider = rustls::crypto::ring::default_provider();
    Ok(Arc::new(CertifiedKey::from_der(chain, key, &provider).context("the certificate and its key don't match")?))
}

/// The certificate in use; `renew` swaps in a new one while connections carry on.
#[derive(Debug)]
pub struct Reloading(RwLock<Arc<CertifiedKey>>);

impl ResolvesServerCert for Reloading {
    fn resolve(&self, _: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        Some(self.0.read().unwrap().clone())
    }
}

/// A TLS setup for `name`, with a certificate fetched now and renewed every day.
pub fn start(name: &str, dir: &Path) -> Result<Arc<rustls::ServerConfig>> {
    fetch(name, dir)?;
    let current = Arc::new(Reloading(RwLock::new(load(dir)?)));
    let (name, dir, renewing) = (name.to_string(), dir.to_path_buf(), current.clone());
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(24 * 3600)).await;
            let (n, d) = (name.clone(), dir.clone());
            let renewed = tokio::task::spawn_blocking(move || fetch(&n, &d).and_then(|_| load(&d))).await;
            match renewed {
                Ok(Ok(key)) => *renewing.0.write().unwrap() = key,
                // Fail loudly; the current certificate keeps working until it expires.
                Ok(Err(e)) => eprintln!("curia-host: could not renew the HTTPS certificate: {e:#}"),
                Err(e) => eprintln!("curia-host: certificate renewal crashed: {e}"),
            }
        }
    });
    let mut config = rustls::ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .context("no TLS versions available")?
        .with_no_client_auth()
        .with_cert_resolver(current);
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(Arc::new(config))
}
