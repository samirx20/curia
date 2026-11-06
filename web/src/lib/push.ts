// Notifications: subscribe this installed app to the host's push key. The browser asks the
// person once; after that the subscription is renewed on every sign-in, so the host always
// has the current one.
import { link } from "./link";

export type PushState = "on" | "off" | "blocked" | "unsupported";

export function pushState(): PushState {
  if (!("serviceWorker" in navigator) || !("PushManager" in window) || !("Notification" in window)) return "unsupported";
  if (Notification.permission === "denied") return "blocked";
  return Notification.permission === "granted" ? "on" : "off";
}

function bytes(b64url: string): Uint8Array<ArrayBuffer> {
  const s = atob(b64url.replace(/-/g, "+").replace(/_/g, "/"));
  const out = new Uint8Array(new ArrayBuffer(s.length));
  for (let i = 0; i < s.length; i++) out[i] = s.charCodeAt(i);
  return out;
}

/** Sends this app's push subscription to the host (making one if needed). */
export async function subscribe(): Promise<void> {
  const { public_key } = await link.call<{ public_key: string }>("push_key");
  const reg = await navigator.serviceWorker.ready;
  let sub = await reg.pushManager.getSubscription();
  const key = bytes(public_key);
  // A subscription made with another host key can't be used: replace it.
  const old = sub?.options.applicationServerKey;
  if (sub && old && new Uint8Array(old).join() !== key.join()) {
    await sub.unsubscribe();
    sub = null;
  }
  sub ??= await reg.pushManager.subscribe({ userVisibleOnly: true, applicationServerKey: key });
  const json = sub.toJSON();
  await link.call("push_subscribe", { endpoint: json.endpoint, p256dh: json.keys?.p256dh, auth: json.keys?.auth });
}

/** Asks the person (needs a tap), then subscribes. */
export async function enable(): Promise<PushState> {
  if (pushState() === "unsupported") return "unsupported";
  const answer = await Notification.requestPermission();
  if (answer === "granted") await subscribe();
  return pushState();
}
