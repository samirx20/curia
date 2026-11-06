// This device's own key (Tech Stack §5): an ECDSA P-256 key pair made by WebCrypto, kept in
// IndexedDB with its private half non-exportable. The host stores only the public half.
// iPhones can clear a web app's data; then the key is gone and the device pairs again.

export interface Identity {
  deviceId: string;
  role: string;
  keys: CryptoKeyPair;
}

const DB = "curia";
const STORE = "device";

function db(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB, 1);
    req.onupgradeneeded = () => req.result.createObjectStore(STORE);
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

async function get<T>(key: string): Promise<T | undefined> {
  const d = await db();
  return new Promise((resolve, reject) => {
    const req = d.transaction(STORE).objectStore(STORE).get(key);
    req.onsuccess = () => resolve(req.result as T | undefined);
    req.onerror = () => reject(req.error);
  });
}

async function put(key: string, value: unknown): Promise<void> {
  const d = await db();
  return new Promise((resolve, reject) => {
    const tx = d.transaction(STORE, "readwrite");
    tx.objectStore(STORE).put(value, key);
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
}

export const identity = (): Promise<Identity | undefined> => get<Identity>("identity");
export const forget = (): Promise<void> => put("identity", undefined);

/** A new key pair; its public half as the host wants it (the uncompressed point, base64). */
export async function newKeys(): Promise<{ keys: CryptoKeyPair; publicKey: string }> {
  const keys = await crypto.subtle.generateKey({ name: "ECDSA", namedCurve: "P-256" }, false, ["sign", "verify"]);
  const raw = new Uint8Array(await crypto.subtle.exportKey("raw", keys.publicKey));
  return { keys, publicKey: b64(raw) };
}

export async function remember(id: Identity): Promise<void> {
  await put("identity", id);
  // Ask the browser to keep this site's data, so the key survives storage pressure.
  await navigator.storage?.persist?.().catch(() => false);
}

/** Signs `message` with the device's key: r‖s, base64, as the host checks it. */
export async function sign(keys: CryptoKeyPair, message: string): Promise<string> {
  const sig = await crypto.subtle.sign({ name: "ECDSA", hash: "SHA-256" }, keys.privateKey, new TextEncoder().encode(message));
  return b64(new Uint8Array(sig));
}

function b64(bytes: Uint8Array): string {
  let s = "";
  for (const b of bytes) s += String.fromCharCode(b);
  return btoa(s);
}

/** A name for this device, from what the browser says it is. */
export function guessName(): string {
  const ua = navigator.userAgent;
  if (/iPhone/.test(ua)) return "iPhone";
  if (/iPad/.test(ua)) return "iPad";
  if (/Android/.test(ua)) return "Android phone";
  if (/Windows/.test(ua)) return "Windows browser";
  if (/Mac/.test(ua)) return "Mac browser";
  return "Browser";
}
