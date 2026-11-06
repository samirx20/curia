// The link to the host: JSON-RPC over the same-origin WebSocket. Signs in with this device's
// key (challenge → hello_device), reconnects after a drop, and replaces the socket when the
// app comes back after more than 20 s in the background (phones starve hidden sockets).
import { identity, sign, type Identity } from "./device";

export type LinkState = "connecting" | "open" | "offline" | "unpaired";
type Waiting = { resolve: (v: any) => void; reject: (e: Error) => void };

export interface WireEvent { thread_id: string; seq: number | null; event: any }

export class Link {
  state: LinkState = "connecting";
  role = "";
  onState: (s: LinkState) => void = () => {};
  onEvent: (e: WireEvent) => void = () => {};
  onActivity: (threadId: string, kind: string) => void = () => {};
  onReady: () => void = () => {};

  private ws: WebSocket | null = null;
  private nextId = 1;
  private waiting = new Map<number, Waiting>();
  private me: Identity | undefined;
  private retry = 1000;
  private hiddenAt = 0;

  constructor() {
    document.addEventListener("visibilitychange", () => {
      if (document.hidden) this.hiddenAt = Date.now();
      else if (this.hiddenAt && Date.now() - this.hiddenAt > 20_000) this.reconnect();
    });
  }

  async start(): Promise<void> {
    this.me = await identity();
    if (!this.me) return this.set("unpaired");
    this.open();
  }

  /** Opens a socket without signing in (for pairing). */
  async bare(): Promise<void> {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) return;
    await new Promise<void>((resolve, reject) => {
      const ws = this.socket();
      ws.addEventListener("open", () => resolve(), { once: true });
      ws.addEventListener("error", () => reject(new Error("could not reach the host")), { once: true });
    });
  }

  call<T = any>(method: string, params?: unknown): Promise<T> {
    const ws = this.ws;
    if (!ws || ws.readyState !== WebSocket.OPEN) return Promise.reject(new Error("not connected to the host"));
    const id = this.nextId++;
    return new Promise<T>((resolve, reject) => {
      this.waiting.set(id, { resolve, reject });
      ws.send(JSON.stringify(params === undefined ? { jsonrpc: "2.0", id, method } : { jsonrpc: "2.0", id, method, params }));
    });
  }

  /** Answers a request, signed with this device's key so the host can record who answered. */
  async answer(threadId: string, requestId: string, decision: { decision: string; [k: string]: unknown }): Promise<void> {
    if (!this.me) throw new Error("this device isn't paired");
    const signature = await sign(this.me.keys, `curia-answer:${threadId}:${requestId}:${decision.decision}`);
    await this.call("thread_answer", { thread_id: threadId, request_id: requestId, decision, signature });
  }

  reconnect(): void {
    this.ws?.close();
  }

  private open(): void {
    const ws = this.socket();
    ws.addEventListener("open", () => this.signIn().catch((e) => this.refused(e)));
  }

  private socket(): WebSocket {
    this.set("connecting");
    const ws = new WebSocket(`${location.protocol === "https:" ? "wss" : "ws"}://${location.host}/ws`);
    this.ws = ws;
    ws.addEventListener("message", (m) => this.receive(m.data));
    ws.addEventListener("close", () => {
      if (this.ws !== ws) return;
      for (const w of this.waiting.values()) w.reject(new Error("the connection to the host closed"));
      this.waiting.clear();
      if (this.state === "unpaired" || !this.me) return;
      this.set("offline");
      setTimeout(() => this.ws === ws && this.open(), this.retry);
      this.retry = Math.min(this.retry * 2, 10_000);
    });
    return ws;
  }

  private async signIn(): Promise<void> {
    const me = this.me!;
    const { nonce } = await this.call<{ nonce: string }>("challenge");
    const signature = await sign(me.keys, `curia-sign-in:${nonce}`);
    const hello = await this.call<{ role: string }>("hello_device", { device_id: me.deviceId, signature });
    this.role = hello.role;
    this.retry = 1000;
    this.set("open");
    this.onReady();
  }

  private refused(e: Error): void {
    // The host no longer knows this device (unpaired, or its data was reset): pair again.
    if (/isn't paired|unpaired|doesn't match/.test(e.message)) this.set("unpaired");
  }

  private receive(data: string): void {
    const msg = JSON.parse(data);
    for (const m of Array.isArray(msg) ? msg : [msg]) {
      if (m.id !== undefined && m.method === undefined) {
        const w = this.waiting.get(m.id);
        if (!w) continue;
        this.waiting.delete(m.id);
        if (m.error) w.reject(new Error(m.error.message));
        else w.resolve(m.result);
      } else if (m.method === "event") this.onEvent(m.params);
      else if (m.method === "activity") this.onActivity(m.params.thread_id, m.params.kind);
    }
  }

  private set(s: LinkState): void {
    this.state = s;
    this.onState(s);
  }
}

export const link = new Link();
