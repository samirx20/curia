//! A client on another machine: it pairs with a host the way a phone does (a one-time code
//! from that host's owner), keeps its own P-256 key, and signs in and signs its answers with it.
use crate::{Incoming, Link};
use anyhow::{Context, Result, bail};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use curia_core::protocol::Call;
use curia_core::signing;
use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::sync::mpsc;

/// A host this client is paired with.
#[derive(Clone, Serialize, Deserialize)]
pub struct Device {
    /// This pairing's id here (the host's device id).
    pub device_id: String,
    /// The host's web address, such as `https://curia.example.ts.net/`.
    pub url: String,
    /// What to call the host (its address's first label until it says otherwise).
    pub host_name: String,
    pub role: String,
    /// The private key, base64 (32 bytes). Kept in the user's own app-data folder.
    key: String,
}

impl Device {
    fn signing_key(&self) -> Result<SigningKey> {
        SigningKey::from_slice(&B64.decode(&self.key)?).context("this host's saved key is damaged; pair again")
    }
}

/// Splits what someone pastes (a pairing link `https://host/#pair=CODE`, or an address and a
/// separate code) into the host's address and the code.
pub fn parse_pairing(link: &str, code: &str) -> Result<(String, String)> {
    let link = link.trim();
    let (base, fragment) = link.split_once('#').unwrap_or((link, ""));
    let code = fragment.strip_prefix("pair=").filter(|c| !c.is_empty()).unwrap_or(code.trim()).to_string();
    if !(base.starts_with("https://") || base.starts_with("http://")) {
        bail!("the host's address starts with https:// (copy it from that host's Settings > Access)");
    }
    if code.is_empty() {
        bail!("a pairing code is needed (that host's Settings > Access makes one)");
    }
    Ok((format!("{}/", base.trim_end_matches('/')), code))
}

/// The link's WebSocket address for a host's web address.
fn ws_url(url: &str) -> String {
    let rest = url.strip_prefix("https://").map(|r| format!("wss://{r}")).or_else(|| url.strip_prefix("http://").map(|r| format!("ws://{r}")));
    format!("{}ws", rest.unwrap_or_else(|| url.to_string()))
}

/// Pairs with the host at `url` using a one-time `code`, as `name`.
pub async fn pair(url: &str, code: &str, name: &str) -> Result<Device> {
    let key = new_key()?;
    let public = B64.encode(key.verifying_key().to_sec1_bytes());
    let (link, _) = Link::open(&ws_url(url)).await?;
    let reply = link.call(Call::Pair { code: code.to_string(), name: name.to_string(), public_key: public }).await.context("the host refused the pairing")?;
    let host_name = url.split("://").nth(1).and_then(|h| h.split(['.', ':', '/']).next()).unwrap_or("host").to_string();
    Ok(Device {
        device_id: reply["device_id"].as_str().context("the host's reply has no device id")?.to_string(),
        role: reply["role"].as_str().unwrap_or("view").to_string(),
        url: url.to_string(),
        host_name,
        key: B64.encode(key.to_bytes()),
    })
}

fn new_key() -> Result<SigningKey> {
    loop {
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).map_err(|e| anyhow::anyhow!("no randomness for a key: {e}"))?;
        if let Ok(key) = SigningKey::from_slice(&bytes) {
            return Ok(key);
        }
    }
}

impl Link {
    /// Connects to a paired host and signs in with this client's key. Answers sent on the
    /// link are signed with it too.
    pub async fn connect_device(device: &Device) -> Result<(Link, mpsc::UnboundedReceiver<Incoming>)> {
        let key = device.signing_key()?;
        let (mut link, events) = Link::open(&ws_url(&device.url)).await?;
        let challenge = link.call(Call::Challenge).await?;
        let nonce = challenge["nonce"].as_str().context("the host sent no challenge")?;
        let signature = sign(&key, &signing::sign_in_message(nonce));
        link.call(Call::HelloDevice { device_id: device.device_id.clone(), signature }).await.context("the host refused this device")?;
        link.signer = Some(key);
        Ok((link, events))
    }
}

pub(crate) fn sign(key: &SigningKey, message: &str) -> String {
    let sig: Signature = key.sign(message.as_bytes());
    B64.encode(sig.to_bytes())
}

/// The hosts this client is paired with, in a JSON file.
pub fn load(path: &Path) -> Vec<Device> {
    std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn save(path: &Path, devices: &[Device]) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, serde_json::to_string_pretty(devices)?).with_context(|| format!("could not save {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pairing_link_carries_its_code() {
        assert_eq!(parse_pairing("https://curia.example.ts.net/#pair=ABC123", "").unwrap(), ("https://curia.example.ts.net/".into(), "ABC123".into()));
        assert_eq!(parse_pairing(" https://h.example:47443 ", "XYZ").unwrap(), ("https://h.example:47443/".into(), "XYZ".into()));
        assert!(parse_pairing("curia.example", "XYZ").is_err());
        assert!(parse_pairing("https://h.example/", "").is_err());
        assert_eq!(ws_url("https://h.example/"), "wss://h.example/ws");
        assert_eq!(ws_url("http://127.0.0.1:4000/"), "ws://127.0.0.1:4000/ws");
    }
}
