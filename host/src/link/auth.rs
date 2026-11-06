//! Who may use the link, and for what (Tech Stack §3). A client on the host itself signs in
//! with the token in `link.json` and is the owner. A paired device signs in by signing a fresh
//! challenge with its own key (ECDSA P-256, which every browser's WebCrypto has); its role
//! (`owner`, `approve`, `view`) is checked on every call. Pairing takes a one-time code that
//! an owner asks for and that lasts a minute.
use anyhow::{Context, Result, bail};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use curia_core::protocol::Call;
use p256::ecdsa::signature::Verifier;
use p256::ecdsa::{Signature, VerifyingKey};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    View,
    Approve,
    Owner,
}

impl Role {
    pub fn parse(name: &str) -> Result<Role> {
        Ok(match name {
            "view" => Role::View,
            "approve" => Role::Approve,
            "owner" => Role::Owner,
            other => bail!("unknown role {other:?} (owner, approve or view)"),
        })
    }
    pub fn name(self) -> &'static str {
        match self {
            Role::View => "view",
            Role::Approve => "approve",
            Role::Owner => "owner",
        }
    }
}

/// Who a connection is, once signed in.
#[derive(Debug, Clone, PartialEq)]
pub enum Who {
    /// A client on the host itself (`link.json`'s token).
    Local,
    Device { id: String, role: Role },
}

impl Who {
    pub fn role(&self) -> Role {
        match self {
            Who::Local => Role::Owner,
            Who::Device { role, .. } => *role,
        }
    }
    /// How the log names who answered.
    pub fn label(&self) -> &str {
        match self {
            Who::Local => "local",
            Who::Device { id, .. } => id,
        }
    }
}

/// How long a pairing code lasts: a minute unless asked for longer, never over ten.
fn pair_for(minutes: Option<u32>) -> Duration {
    Duration::from_secs(60 * u64::from(minutes.unwrap_or(1).clamp(1, 10)))
}

/// What every connection shares: the local token, the current pairing code, and the news
/// that a device was revoked (its connections close).
pub struct Gate {
    pub token: String,
    /// Where phones reach the host, for pairing addresses (none: this machine only).
    pub public_url: Option<String>,
    pair: Mutex<Option<(String, Role, Instant)>>,
    pub revoked: broadcast::Sender<String>,
}

impl Gate {
    pub fn new(token: String) -> Gate {
        Gate { token, public_url: None, pair: Mutex::new(None), revoked: broadcast::channel(16).0 }
    }

    /// A new one-time code for a device with `role`; any earlier code stops working.
    pub fn pair_code(&self, role: Role, minutes: Option<u32>) -> (String, i64) {
        let code: String = uuid::Uuid::new_v4().simple().to_string()[..10].to_uppercase();
        let ttl = pair_for(minutes);
        *self.pair.lock().unwrap() = Some((code.clone(), role, Instant::now() + ttl));
        (code, crate::store::now_ms() + ttl.as_millis() as i64)
    }

    /// Uses up the code: the role it was made for, or an error.
    pub fn redeem(&self, code: &str) -> Result<Role> {
        let mut pair = self.pair.lock().unwrap();
        match pair.take() {
            Some((c, role, until)) if c == code.trim().to_uppercase() && Instant::now() < until => Ok(role),
            Some((c, role, until)) if Instant::now() < until && c != code.trim().to_uppercase() => {
                *pair = Some((c, role, until)); // a wrong guess doesn't burn the real code
                bail!("that pairing code isn't right")
            }
            _ => bail!("that pairing code has expired; ask for a new one"),
        }
    }
}

/// A fresh challenge for one sign-in.
pub fn nonce() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

pub use curia_core::signing::{answer_message, sign_in_message};

/// Checks a device's signature of `message`. `public_key`: the uncompressed P-256 point;
/// `signature`: r‖s (64 bytes), as WebCrypto makes them; both base64.
pub fn verify(public_key: &str, message: &str, signature: &str) -> Result<()> {
    let key = B64.decode(public_key.trim()).context("the device key isn't base64")?;
    let key = VerifyingKey::from_sec1_bytes(&key).context("the device key isn't a P-256 public key")?;
    let sig = B64.decode(signature.trim()).context("the signature isn't base64")?;
    let sig = Signature::from_slice(&sig).context("the signature isn't a P-256 signature")?;
    key.verify(message.as_bytes(), &sig).context("the signature doesn't match the device's key")
}

/// Checks that a public key is usable before a device is stored with it.
pub fn check_key(public_key: &str) -> Result<()> {
    let key = B64.decode(public_key.trim()).context("the device key isn't base64")?;
    VerifyingKey::from_sec1_bytes(&key).context("the device key isn't a P-256 public key")?;
    Ok(())
}

/// The role a call needs. Reading needs `view`; answering requests needs `approve`;
/// everything that starts work or changes the host needs `owner`.
pub fn required(call: &Call) -> Role {
    match call {
        Call::ThreadsList | Call::ThreadOpen { .. } | Call::ThreadHistory { .. } | Call::TeamsList | Call::SettingsGet
        | Call::InboxList | Call::ThreadFiles { .. } | Call::FileRead { .. } | Call::FileDiff { .. }
        | Call::PushKey | Call::PushSubscribe { .. } | Call::TerminalList { .. } | Call::TerminalOpen { .. }
        | Call::ThreadTree { .. } | Call::FileLink { .. } | Call::ThreadPorts { .. }
        | Call::Search { .. } | Call::RunSummary { .. } => Role::View,
        Call::ThreadAnswer { .. } => Role::Approve,
        _ => Role::Owner,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use curia_core::Decision;
    use p256::ecdsa::SigningKey;
    use p256::ecdsa::signature::Signer;

    fn device() -> (SigningKey, String) {
        let sk = SigningKey::from_slice(&[7u8; 32]).unwrap();
        let public = B64.encode(sk.verifying_key().to_sec1_bytes());
        (sk, public)
    }

    #[test]
    fn a_device_signs_in_by_signing_the_challenge() {
        let (sk, public) = device();
        let n = nonce();
        let sig: Signature = sk.sign(sign_in_message(&n).as_bytes());
        let sig = B64.encode(sig.to_bytes());
        verify(&public, &sign_in_message(&n), &sig).unwrap();
        assert!(verify(&public, &sign_in_message("another"), &sig).is_err(), "an old signature doesn't sign a new challenge");
    }

    #[test]
    fn a_pairing_code_works_once_and_a_wrong_guess_keeps_it() {
        let gate = Gate::new("t".into());
        let (code, _) = gate.pair_code(Role::Approve, None);
        assert_eq!(pair_for(Some(60)), Duration::from_secs(600), "never more than ten minutes");
        assert!(gate.redeem("WRONG").is_err());
        assert_eq!(gate.redeem(&code.to_lowercase()).unwrap(), Role::Approve);
        assert!(gate.redeem(&code).is_err(), "used up");
    }

    #[test]
    fn roles_cover_reading_answering_and_running() {
        assert_eq!(required(&Call::InboxList), Role::View);
        let answer = Call::ThreadAnswer { thread_id: "t".into(), request_id: "r".into(), decision: Decision::Allow, signature: None };
        assert_eq!(required(&answer), Role::Approve);
        assert_eq!(required(&Call::ThreadSend { thread_id: "t".into(), text: "x".into() }), Role::Owner);
        assert!(Role::Owner > Role::Approve && Role::Approve > Role::View);
    }
}
