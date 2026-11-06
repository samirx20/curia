// The installable app's service worker. For now it only makes the app installable and
// network-first (the host is the source of truth); push notifications come next.
self.addEventListener("install", () => self.skipWaiting());
self.addEventListener("activate", (e) => e.waitUntil(self.clients.claim()));
self.addEventListener("fetch", () => {});
