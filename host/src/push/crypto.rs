//! Web Push's two pieces of cryptography, done with RustCrypto:
//! - the message encryption of RFC 8291 (`aes128gcm`): only the phone can read a notification,
//!   not Google's or Apple's push service that carries it;
//! - the VAPID signature of RFC 8292: the push service knows the message is from this host.
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes128Gcm, Nonce};
use anyhow::{Context, Result, bail};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64URL;
use hkdf::Hkdf;
use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use p256::{PublicKey, SecretKey};
use sha2::Sha256;

/// One record holds the whole message; 4096 is what every browser expects.
const RECORD_SIZE: u32 = 4096;

pub fn random<const N: usize>() -> Result<[u8; N]> {
    let mut b = [0u8; N];
    getrandom::fill(&mut b).map_err(|e| anyhow::anyhow!("no randomness from the system: {e}"))?;
    Ok(b)
}

/// A fresh P-256 private key.
pub fn new_key() -> Result<SecretKey> {
    loop {
        if let Ok(k) = SecretKey::from_slice(&random::<32>()?) {
            return Ok(k);
        }
    }
}

/// The uncompressed public point (65 bytes), as browsers and push services want it.
pub fn public_bytes(key: &SecretKey) -> Vec<u8> {
    key.public_key().to_sec1_bytes().into_vec()
}

/// Encrypts `plaintext` for one subscription (its `p256dh` key and `auth` secret, base64url).
pub fn encrypt(plaintext: &[u8], p256dh: &str, auth: &str) -> Result<Vec<u8>> {
    encrypt_with(plaintext, p256dh, auth, &new_key()?, random::<16>()?)
}

fn encrypt_with(plaintext: &[u8], p256dh: &str, auth: &str, sender: &SecretKey, salt: [u8; 16]) -> Result<Vec<u8>> {
    let ua_public = B64URL.decode(p256dh.trim_end_matches('=')).context("the subscription's p256dh isn't base64url")?;
    let auth = B64URL.decode(auth.trim_end_matches('=')).context("the subscription's auth isn't base64url")?;
    let ua = PublicKey::from_sec1_bytes(&ua_public).context("the subscription's p256dh isn't a P-256 key")?;
    let as_public = public_bytes(sender);
    if as_public.len() != 65 || ua_public.len() != 65 {
        bail!("Web Push needs uncompressed P-256 keys");
    }
    let shared = sender.diffie_hellman(&ua);

    // IKM = HKDF(auth, shared secret, "WebPush: info" ‖ 0 ‖ ua_public ‖ as_public)
    let mut info = b"WebPush: info\0".to_vec();
    info.extend_from_slice(&ua_public);
    info.extend_from_slice(&as_public);
    let mut ikm = [0u8; 32];
    Hkdf::<Sha256>::new(Some(&auth), shared.raw_secret_bytes()).expand(&info, &mut ikm).map_err(|_| anyhow::anyhow!("HKDF failed"))?;

    // The content key and nonce, from the salt.
    let prk = Hkdf::<Sha256>::new(Some(&salt), &ikm);
    let (mut cek, mut nonce) = ([0u8; 16], [0u8; 12]);
    prk.expand(b"Content-Encoding: aes128gcm\0", &mut cek).map_err(|_| anyhow::anyhow!("HKDF failed"))?;
    prk.expand(b"Content-Encoding: nonce\0", &mut nonce).map_err(|_| anyhow::anyhow!("HKDF failed"))?;

    // One record: the message, then the "last record" delimiter.
    let mut record = plaintext.to_vec();
    record.push(2);
    if record.len() + 16 > RECORD_SIZE as usize - 86 {
        bail!("the notification is too long to send");
    }
    let cipher = Aes128Gcm::new_from_slice(&cek).map_err(|_| anyhow::anyhow!("bad content key"))?;
    let sealed = cipher.encrypt(&Nonce::from(nonce), record.as_slice()).map_err(|_| anyhow::anyhow!("encryption failed"))?;

    // Header: salt ‖ record size ‖ key id length ‖ key id (the sender's public key).
    let mut body = salt.to_vec();
    body.extend_from_slice(&RECORD_SIZE.to_be_bytes());
    body.push(65);
    body.extend_from_slice(&as_public);
    body.extend_from_slice(&sealed);
    Ok(body)
}

/// The VAPID `Authorization` header for a push service at `audience` (its origin), signed
/// with the host's push key and good for 12 hours.
pub fn vapid(key: &SecretKey, audience: &str, now_secs: u64) -> Result<String> {
    let header = B64URL.encode(br#"{"typ":"JWT","alg":"ES256"}"#);
    // `sub` says who sends; a project page rather than anyone's email address.
    let claims = serde_json::json!({"aud": audience, "exp": now_secs + 12 * 3600, "sub": "https://github.com/samirx20/curia"});
    let claims = B64URL.encode(claims.to_string());
    let signing = SigningKey::from(key.clone());
    let sig: Signature = signing.sign(format!("{header}.{claims}").as_bytes());
    let jwt = format!("{header}.{claims}.{}", B64URL.encode(sig.to_bytes()));
    Ok(format!("vapid t={jwt}, k={}", B64URL.encode(public_bytes(key))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use p256::ecdsa::VerifyingKey;
    use p256::ecdsa::signature::Verifier;

    /// RFC 8291, Appendix A: the worked example, byte for byte.
    #[test]
    fn matches_the_rfc_example() {
        let sender = SecretKey::from_slice(&B64URL.decode("yfWPiYE-n46HLnH0KqZOF1fJJU3MYrct3AELtAQ-oRw").unwrap()).unwrap();
        let salt: [u8; 16] = B64URL.decode("DGv6ra1nlYgDCS1FRnbzlw").unwrap().try_into().unwrap();
        let body = encrypt_with(
            b"When I grow up, I want to be a watermelon",
            "BCVxsr7N_eNgVRqvHtD0zTZsEc6-VV-JvLexhqUzORcxaOzi6-AYWXvTBHm4bjyPjs7Vd8pZGH6SRpkNtoIAiw4",
            "BTBZMqHH6r4Tts7J_aSIgg",
            &sender,
            salt,
        )
        .unwrap();
        assert_eq!(
            B64URL.encode(body),
            "DGv6ra1nlYgDCS1FRnbzlwAAEABBBP4z9KsN6nGRTbVYI_c7VJSPQTBtkgcy27mlmlMoZIIgDll6e3vCYLocInmYWAmS6TlzAC8wEqKK6PBru3jl7A_yl95bQpu6cVPTpK4Mqgkf1CXztLVBSt2Ks3oZwbuwXPXLWyouBWLVWGNWQexSgSxsj_Qulcy4a-fN"
        );
    }

    #[test]
    fn the_vapid_token_is_signed_by_the_host_key() {
        let key = new_key().unwrap();
        let header = vapid(&key, "https://fcm.googleapis.com", 1_800_000_000).unwrap();
        let jwt = header.strip_prefix("vapid t=").unwrap().split(", k=").next().unwrap();
        let (signed, sig) = jwt.rsplit_once('.').unwrap();
        let sig = Signature::from_slice(&B64URL.decode(sig).unwrap()).unwrap();
        VerifyingKey::from(&SigningKey::from(key)).verify(signed.as_bytes(), &sig).unwrap();
        let claims: serde_json::Value = serde_json::from_slice(&B64URL.decode(signed.split('.').nth(1).unwrap()).unwrap()).unwrap();
        assert_eq!(claims["aud"], "https://fcm.googleapis.com");
    }
}
