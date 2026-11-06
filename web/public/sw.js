// Curia's service worker: makes the app installable and opens it offline, shows the host's notifications, and
// answers a request from the notification's Allow / Deny buttons (Android). An answer is
// signed with this device's key from IndexedDB and sent over the same link the app uses.

// The app's own files are fetched from the host first (it's the source of truth) and kept,
// so when the host can't be reached the app still opens and says so, instead of hanging.
const SHELL = "curia-shell";
self.addEventListener("install", (e) => {
  self.skipWaiting();
  e.waitUntil(caches.open(SHELL).then((c) => c.addAll(["/", "/manifest.webmanifest", "/icon-192.png"])).catch(() => {}));
});
self.addEventListener("activate", (e) => e.waitUntil(self.clients.claim()));
self.addEventListener("fetch", (e) => {
  const url = new URL(e.request.url);
  if (e.request.method !== "GET" || url.origin !== self.location.origin || url.pathname === "/ws") return;
  e.respondWith((async () => {
    const cache = await caches.open(SHELL);
    try {
      const fresh = await fetch(e.request);
      if (fresh.ok) cache.put(e.request.mode === "navigate" ? "/" : e.request, fresh.clone());
      return fresh;
    } catch {
      const kept = await cache.match(e.request.mode === "navigate" ? "/" : e.request);
      return kept || new Response("Can't reach the Curia host. Is Tailscale on?", { status: 503, headers: { "Content-Type": "text/plain" } });
    }
  })());
});

self.addEventListener("push", (e) => {
  let m = {};
  try { m = e.data ? e.data.json() : {}; } catch { m = { title: "Curia", body: e.data ? e.data.text() : "" }; }
  const actions = m.request_id && !m.question ? [{ action: "allow", title: "Allow" }, { action: "deny", title: "Deny" }] : [];
  e.waitUntil(self.registration.showNotification(m.title || "Curia", {
    body: m.body || "",
    tag: m.request_id ? `req-${m.thread_id}-${m.request_id}` : undefined,
    icon: "/icon-192.png",
    badge: "/icon-192.png",
    data: m,
    actions,
    requireInteraction: true,
  }));
});

self.addEventListener("notificationclick", (e) => {
  const m = e.notification.data || {};
  e.notification.close();
  if ((e.action === "allow" || e.action === "deny") && m.request_id) {
    e.waitUntil(answer(m.thread_id, m.request_id, e.action).catch((err) =>
      // Couldn't answer from here (say, the phone isn't on Tailscale): open the request instead.
      self.registration.showNotification("Couldn't send your answer", { body: String(err.message || err), data: m, tag: "curia-error" })));
    return;
  }
  e.waitUntil(open(m.thread_id ? `/#/t/${m.thread_id}` : "/"));
});

async function open(url) {
  const wins = await self.clients.matchAll({ type: "window", includeUncontrolled: true });
  for (const w of wins) {
    if ("focus" in w) { await w.navigate(url).catch(() => {}); return w.focus(); }
  }
  return self.clients.openWindow(url);
}

function identity() {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open("curia", 1);
    req.onupgradeneeded = () => req.result.createObjectStore("device");
    req.onerror = () => reject(req.error);
    req.onsuccess = () => {
      const get = req.result.transaction("device").objectStore("device").get("identity");
      get.onsuccess = () => (get.result ? resolve(get.result) : reject(new Error("this device isn't paired")));
      get.onerror = () => reject(get.error);
    };
  });
}

async function sign(keys, message) {
  const sig = await crypto.subtle.sign({ name: "ECDSA", hash: "SHA-256" }, keys.privateKey, new TextEncoder().encode(message));
  return btoa(String.fromCharCode(...new Uint8Array(sig)));
}

/** Signs in with the device key and answers one request, then closes. */
async function answer(threadId, requestId, decision) {
  const me = await identity();
  const ws = new WebSocket(`${self.location.protocol === "https:" ? "wss" : "ws"}://${self.location.host}/ws`);
  let next = 1;
  const waiting = new Map();
  const call = (method, params) => new Promise((resolve, reject) => {
    const id = next++;
    waiting.set(id, { resolve, reject });
    ws.send(JSON.stringify(params === undefined ? { jsonrpc: "2.0", id, method } : { jsonrpc: "2.0", id, method, params }));
  });
  ws.onmessage = (m) => {
    for (const r of [].concat(JSON.parse(m.data))) {
      const w = r.id !== undefined && r.method === undefined ? waiting.get(r.id) : undefined;
      if (!w) continue;
      waiting.delete(r.id);
      r.error ? w.reject(new Error(r.error.message)) : w.resolve(r.result);
    }
  };
  await new Promise((resolve, reject) => {
    ws.onopen = resolve;
    ws.onerror = () => reject(new Error("can't reach the host; is Tailscale on?"));
    setTimeout(() => reject(new Error("the host didn't answer in time")), 10000);
  });
  try {
    const { nonce } = await call("challenge");
    await call("hello_device", { device_id: me.deviceId, signature: await sign(me.keys, `curia-sign-in:${nonce}`) });
    const d = decision === "allow" ? { decision: "allow" } : { decision: "deny", message: null };
    const signature = await sign(me.keys, `curia-answer:${threadId}:${requestId}:${decision}`);
    await call("thread_answer", { thread_id: threadId, request_id: requestId, decision: d, signature });
  } finally {
    ws.close();
  }
}
