//! The calls about who is connected: signing in, pairing, the device list, and checking that
//! a device's answer carries its signature.
use super::auth::{self, Gate, Role, Who};
use crate::host::Host;
use anyhow::{Result, bail};
use curia_core::protocol::Call;
use serde_json::{Value, json};

/// A call before sign-in. Answers with the reply and, once signed in, who this is.
/// An error closes the connection unless `retry` says the client may try again.
pub struct Refused {
    pub message: String,
    pub retry: bool,
}

pub fn greet(host: &Host, gate: &Gate, call: &Call, nonce: &mut Option<String>) -> Result<(Value, Option<Who>), Refused> {
    let refuse = |message: String, retry: bool| Refused { message, retry };
    match call {
        Call::Hello { token } if *token == gate.token => Ok((hello(), Some(Who::Local))),
        Call::Hello { .. } => Err(refuse("that token isn't this host's".into(), false)),
        Call::Challenge => {
            let n = auth::nonce();
            *nonce = Some(n.clone());
            Ok((json!({"nonce": n}), None))
        }
        Call::HelloDevice { device_id, signature } => {
            let Some(n) = nonce.take() else { return Err(refuse("ask for a challenge first".into(), true)) };
            let device = host.store.device(device_id).map_err(|e| refuse(format!("{e:#}"), false))?
                .ok_or_else(|| refuse("this device isn't paired with the host (or was unpaired); pair it again".into(), false))?;
            auth::verify(&device.public_key, &auth::sign_in_message(&n), signature).map_err(|e| refuse(format!("{e:#}"), false))?;
            let role = Role::parse(&device.role).map_err(|e| refuse(format!("{e:#}"), false))?;
            let _ = host.store.device_seen(&device.id);
            Ok((json!({"host": "curia", "version": env!("CARGO_PKG_VERSION"), "device": device.name, "role": role.name()}),
                Some(Who::Device { id: device.id, role })))
        }
        Call::Pair { code, name, public_key } => {
            let paired = (|| -> Result<Value> {
                auth::check_key(public_key)?;
                let role = gate.redeem(code)?;
                let name = if name.trim().is_empty() { "A device" } else { name.trim() };
                let device = host.store.device_add(name, role.name(), public_key.trim())?;
                Ok(json!({"device_id": device.id, "role": device.role}))
            })();
            // Pairing doesn't sign in: the device then signs a challenge like any other time.
            paired.map(|v| (v, None)).map_err(|e| refuse(format!("{e:#}"), true))
        }
        _ => Err(refuse("sign in first (hello, or challenge then hello_device)".into(), false)),
    }
}

fn hello() -> Value {
    json!({"host": "curia", "version": env!("CARGO_PKG_VERSION"), "role": "owner"})
}

/// The device calls (roles checked before this). None for any other call.
pub fn devices(host: &Host, gate: &Gate, who: &Who, call: &Call) -> Option<Result<Value>> {
    Some(match call {
        Call::PushKey => Ok(json!({"public_key": host.push.public})),
        Call::PushSubscribe { endpoint, p256dh, auth } => match who {
            Who::Device { id, .. } if endpoint.starts_with("https://") => host.store.push_subscribe(id, endpoint, p256dh, auth).map(|_| Value::Null),
            Who::Device { .. } => Err(anyhow::anyhow!("a push endpoint must be https")),
            Who::Local => Err(anyhow::anyhow!("only a paired device can receive notifications")),
        },
        Call::PairCode { role, minutes } => Role::parse(role).and_then(|r| {
            let (code, expires) = gate.pair_code(r, *minutes);
            // The address a phone opens (the code in its fragment), and the same as a QR code.
            let url = gate.public_url.as_ref().map(|u| format!("{u}#pair={code}"));
            let qr = url.as_deref().map(crate::host::qr).transpose()?;
            Ok(json!({"code": code, "role": r.name(), "expires_at_ms": expires, "url": url, "qr": qr}))
        }),
        Call::DeviceSetRole { device_id, role } => host.store.device_set_role(device_id, role).map(|_| Value::Null),
        Call::DevicesList => host.store.devices().map(|d| json!(d)),
        Call::DeviceRevoke { device_id } => host.store.device_revoke(device_id).and_then(|found| {
            if !found {
                bail!("no device {device_id}");
            }
            let _ = gate.revoked.send(device_id.clone());
            Ok(Value::Null)
        }),
        _ => return None,
    })
}

/// A device's answer must be signed by its key, so the log can say who approved what.
pub fn check_answer(host: &Host, who: &Who, call: &Call) -> Result<()> {
    let (Who::Device { id, .. }, Call::ThreadAnswer { thread_id, request_id, decision, signature }) = (who, call) else { return Ok(()) };
    let Some(signature) = signature else { bail!("a device's answer must be signed with its key") };
    let device = host.store.device(id)?.ok_or_else(|| anyhow::anyhow!("this device was unpaired"))?;
    auth::verify(&device.public_key, &auth::answer_message(thread_id, request_id, decision), signature)
}
