//! Paired devices end to end over a real socket: pairing, signing in with a key, roles,
//! signed answers, and revoking.
use crate::host::Host;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use curia_client::Link;
use curia_core::Decision;
use curia_core::protocol::Call;
use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use std::time::Duration;

fn sign(sk: &SigningKey, message: &str) -> String {
    let sig: Signature = sk.sign(message.as_bytes());
    B64.encode(sig.to_bytes())
}

async fn sign_in(url: &str, sk: &SigningKey, device_id: &str) -> anyhow::Result<Link> {
    let (link, _) = Link::open(url).await?;
    let challenge = link.call(Call::Challenge).await?;
    let nonce = challenge["nonce"].as_str().unwrap().to_string();
    let signature = sign(sk, &format!("curia-sign-in:{nonce}"));
    link.call(Call::HelloDevice { device_id: device_id.into(), signature }).await?;
    Ok(link)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_device_pairs_signs_in_and_is_held_to_its_role() {
    let data = std::env::temp_dir().join(format!("curia-devices-test-{}", crate::store::new_id()));
    let (host, _) = Host::open(&data).unwrap();
    let (_stop, stopped) = tokio::sync::oneshot::channel::<()>();
    tokio::spawn(super::serve(host.clone(), 0, async move {
        let _ = stopped.await;
    }));
    for _ in 0..50 {
        if data.join("link.json").exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let link: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(data.join("link.json")).unwrap()).unwrap();
    let url = link["url"].as_str().unwrap().to_string();
    let (owner, _) = Link::connect_local(&data).await.unwrap();

    // Pairing: a one-time code from the owner, then the device's public key.
    let code = owner.call(Call::PairCode { role: "approve".into() }).await.unwrap()["code"].as_str().unwrap().to_string();
    let sk = SigningKey::from_slice(&[9u8; 32]).unwrap();
    let public_key = B64.encode(sk.verifying_key().to_sec1_bytes());
    let (phone, _) = Link::open(&url).await.unwrap();
    let paired = phone.call(Call::Pair { code: code.clone(), name: "Sam's phone".into(), public_key: public_key.clone() }).await.unwrap();
    let device_id = paired["device_id"].as_str().unwrap().to_string();
    assert_eq!(paired["role"], "approve");
    let (thief, _) = Link::open(&url).await.unwrap();
    assert!(thief.call(Call::Pair { code, name: "x".into(), public_key: public_key.clone() }).await.is_err(), "a code works once");

    // Signing in: the device signs a fresh challenge; someone else's key can't.
    let phone = sign_in(&url, &sk, &device_id).await.unwrap();
    let stranger = SigningKey::from_slice(&[3u8; 32]).unwrap();
    assert!(sign_in(&url, &stranger, &device_id).await.is_err());

    // Its role: it may look and answer, not start work.
    phone.call(Call::ThreadsList).await.unwrap();
    let send = phone.call(Call::ThreadSend { thread_id: "t".into(), text: "hi".into() }).await;
    assert!(format!("{:#}", send.unwrap_err()).contains("only look and answer"));
    // An unsigned answer is refused before anything else happens.
    let unsigned = phone.call(Call::ThreadAnswer { thread_id: "t".into(), request_id: "r".into(), decision: Decision::Allow, signature: None }).await;
    assert!(format!("{:#}", unsigned.unwrap_err()).contains("must be signed"));

    // Revoking: its open connection closes and it can't sign in again.
    owner.call(Call::DeviceRevoke { device_id: device_id.clone() }).await.unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(phone.call(Call::ThreadsList).await.is_err(), "the revoked device's connection is closed");
    assert!(sign_in(&url, &sk, &device_id).await.is_err());
    assert!(host.store.devices().unwrap().is_empty());
    let _ = std::fs::remove_dir_all(&data);
}
