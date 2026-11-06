<script lang="ts">
  // One thread (docs: Design 31): its task, what it said and did, and the requests waiting on
  // you, answered here with this device's signature. Reopens from its last event after a drop.
  import { onDestroy, tick } from "svelte";
  import Icon from "../lib/Icon.svelte";
  import Badge from "../lib/Badge.svelte";
  import Status from "../lib/Status.svelte";
  import { link } from "../lib/link";
  import { app, needsYou, onSignIn, soon, teamOf } from "../lib/state.svelte";
  import { Rows, asked, kindIcon, summary } from "../lib/rows.svelte";
  import { who } from "../lib/icons";

  let { id }: { id: string } = $props();
  let model = $state(new Rows());
  let last: number | null = null;
  let error = $state("");
  let draft = $state("");
  let list: HTMLElement | undefined = $state();
  let open: Record<number, boolean> = $state({});

  const thread = $derived(app.threads.find((t) => t.id === id));
  const parent = $derived(thread?.parent_id ? app.threads.find((t) => t.id === thread.parent_id) : null);
  const titleOf = (tid: string) => app.threads.find((t) => t.id === tid)?.title ?? "another thread";
  const owner = $derived(app.role === "owner");

  async function load(after: number | null) {
    error = "";
    try {
      const r = await link.call("thread_open", { thread_id: id, after_seq: after });
      if (r.kind === "snapshot") { model = new Rows(); last = null; }
      for (const w of r.events) take(w);
      await tick();
      list?.scrollTo({ top: list.scrollHeight });
    } catch (e) {
      error = (e as Error).message;
    }
  }
  function take(w: any) {
    if (w.thread_id !== id) return;
    if (w.seq !== null) {
      if (last !== null && w.seq <= last) return;
      last = w.seq;
    }
    model.add(w.event);
  }

  const catchUp = () => load(last);
  onSignIn.add(catchUp);
  link.onEvent = (w) => {
    take(w);
    if (w.thread_id === id && list && list.scrollHeight - list.scrollTop - list.clientHeight < 120) tick().then(() => list?.scrollTo({ top: list.scrollHeight }));
    if (["turn_started", "turn_finished", "approval_requested", "approval_resolved"].includes(w.event.type)) soon();
  };
  onDestroy(() => { onSignIn.delete(catchUp); link.onEvent = () => {}; });
  // A new thread id starts fresh; until signed in, the sign-in hook loads it.
  $effect(() => {
    void id;
    model = new Rows();
    last = null;
    if (link.state === "open") load(null);
  });

  async function answer(requestId: string, decision: any) {
    try {
      await link.answer(id, requestId, decision);
      soon();
    } catch (e) {
      error = (e as Error).message;
    }
  }
  async function send() {
    const text = draft.trim();
    if (!text) return;
    try {
      await link.call("thread_send", { thread_id: id, text });
      draft = "";
    } catch (e) {
      error = (e as Error).message;
    }
  }
</script>

<div class="screen">
  <header>
    <a class="back" href="#/" aria-label="Back"><Icon name="back" size={22} /></a>
    <div class="head">
      <div class="name">{thread?.title ?? "Thread"}</div>
      <div class="muted sub">{who(teamOf(thread))}{parent ? ` · for ${parent.title}` : ""}</div>
    </div>
    {#if needsYou(id)}<Status state="needs" size={14} />{:else if thread?.status === "running"}<Status state="running" />{/if}
  </header>

  <div class="scroll body" bind:this={list}>
    {#each model.rows as row, i}
      {#if row.kind === "user"}
        <div class="user">{row.text}</div>
      {:else if row.kind === "task"}
        <div class="card box">
          <div class="cardhead"><Icon name="handIn" size={15} color="var(--purple)" /><span class="muted">Task from</span><b>{titleOf(row.from)}</b></div>
          <div class="text">{row.text}</div>
        </div>
      {:else if row.kind === "text"}
        {#if row.text.trim()}<div class="text">{row.text}</div>{/if}
      {:else if row.kind === "thinking"}
        {#if row.text.trim()}<div class="thinking">{row.text}</div>{/if}
      {:else if row.kind === "tools"}
        <button class="tools" onclick={() => (open[i] = !open[i])}>
          <Icon name={kindIcon(row.steps[0]?.name ?? "")} size={15} color="var(--text3)" />
          <span>{summary(row.steps)}</span>
          {#if row.steps.some((s) => s.state === "running")}<Status state="running" size={13} />{/if}
          <Icon name={open[i] ? "down" : "open"} size={13} color="var(--text3)" />
        </button>
        {#if open[i]}
          {#each row.steps as s}
            <div class="step"><Status state={s.state === "error" ? "failed" : s.state === "running" ? "running" : "done"} size={12} /><code>{s.name.replace(/^mcp__[^_]+__/, "")} {s.input?.command ?? s.input?.file_path ?? s.input?.Path ?? ""}</code></div>
          {/each}
        {/if}
      {:else if row.kind === "approval"}
        <div class="card box ask" class:pending={row.phase === "pending"}>
          {#if row.phase === "pending"}
            <div class="cardhead"><Status state="needs" size={13} /><b class="amber">Needs you</b></div>
          {:else}
            <div class="cardhead"><Status state={row.phase === "allowed" || row.phase === "answered" ? "done" : "failed"} size={13} /><span class="muted">{row.phase}</span></div>
          {/if}
          <div class="text">{row.e.kind === "question" ? "Asks you" : "Wants to run"} <span class="muted">· {row.e.tool_name.replace(/^mcp__([^_]+)__/, "$1: ")}</span></div>
          <div class="well">{asked(row.e.input)}</div>
          {#if row.phase === "pending" && app.role !== "view" && row.e.kind !== "question"}
            <div class="two">
              <button class="btn primary" onclick={() => answer(row.requestId, { decision: "allow" })}>Allow</button>
              <button class="btn secondary" onclick={() => answer(row.requestId, { decision: "deny", message: null })}>Deny</button>
            </div>
          {/if}
        </div>
      {:else if row.kind === "handoff"}
        <a class="card box link" href="#/t/{row.e.thread_id}">
          <div class="cardhead"><Icon name="handOut" size={15} color="var(--purple)" /><span class="muted">Handed to</span><Badge team={app.teams.find((t) => t.name === row.e.team)} size={18} /><b>{who(app.teams.find((t) => t.name === row.e.team))}</b></div>
          <div class="text">{row.e.title}</div>
        </a>
      {:else if row.kind === "handback"}
        <a class="card box link" href="#/t/{row.e.from}">
          <div class="cardhead"><Icon name="handIn" size={15} color="var(--purple)" /><span class="muted">Result from</span><b>{titleOf(row.e.from)}</b></div>
          <div class="text">{row.e.summary}</div>
          <div class="merged" class:undone={row.phase === "undone"}>
            {#if row.phase === "merged"}<Icon name="check" size={14} />Merged into the folder{:else if row.phase === "undone"}Merge undone{:else if row.phase === "failed"}Not merged{/if}
          </div>
        </a>
      {:else if row.kind === "message"}
        <div class="card box"><div class="cardhead"><Icon name="message" size={15} color="var(--purple)" /><span class="muted">Message from</span><b>{titleOf(row.e.from)}</b></div><div class="text">{row.e.text}</div></div>
      {:else if row.kind === "failed"}
        <div class="failed"><Icon name="close" size={15} stroke={2.2} color="var(--red)" />{row.text}</div>
      {:else}
        <div class="notice">{row.text}</div>
      {/if}
    {/each}
    {#if error}<div class="failed"><Icon name="warning" size={15} color="var(--red)" />{error}</div>{/if}
  </div>

  {#if owner}
    <form class="composer" onsubmit={(e) => { e.preventDefault(); send(); }}>
      <input bind:value={draft} placeholder={thread?.parent_id ? "Message this team…" : "Message the main thread…"} />
      <button class="sendbtn" class:ready={draft.trim() !== ""} aria-label="Send"><Icon name="send" size={20} stroke={2.2} /></button>
    </form>
  {/if}
</div>

<style>
  header { display: flex; align-items: center; gap: 10px; padding: calc(12px + env(safe-area-inset-top)) 16px 12px 8px; border-bottom: 1px solid var(--line); }
  .back { color: var(--text); padding: 6px; }
  .head { flex: 1; min-width: 0; }
  .name { font-size: 17px; font-weight: 700; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .sub { font-size: 13px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .body { display: flex; flex-direction: column; gap: 14px; padding-top: 16px; }
  .user { align-self: flex-end; max-width: 85%; background: var(--selection); border-radius: var(--radius); padding: 10px 14px; white-space: pre-wrap; }
  .text { white-space: pre-wrap; overflow-wrap: anywhere; }
  .thinking { color: var(--text3); font-style: italic; font-size: 14px; display: -webkit-box; -webkit-line-clamp: 3; line-clamp: 3; -webkit-box-orient: vertical; overflow: hidden; }
  .box { padding: 12px 14px; display: flex; flex-direction: column; gap: 8px; background: var(--card); }
  .cardhead { display: flex; align-items: center; gap: 8px; font-size: 14px; }
  .cardhead b { font-weight: 600; }
  .link { color: inherit; text-decoration: none; }
  .ask.pending { border-color: var(--amber); }
  .amber { color: var(--amber); }
  .two { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; }
  .tools { display: flex; align-items: center; gap: 8px; color: var(--text2); font-size: 14px; text-align: left; }
  .step { display: flex; align-items: center; gap: 8px; padding-left: 24px; color: var(--text2); min-width: 0; }
  .step code { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; }
  .merged { display: flex; align-items: center; gap: 6px; color: var(--green); font-size: 13px; }
  .merged.undone { color: var(--text3); }
  .notice { color: var(--text3); font-size: 13px; }
  .failed { display: flex; gap: 8px; align-items: flex-start; color: var(--red); }
  .composer { display: flex; gap: 10px; padding: 10px 12px calc(10px + env(safe-area-inset-bottom)); border-top: 1px solid var(--line); }
  .composer input { flex: 1; height: 46px; border-radius: 23px; border: 1px solid var(--line); background: var(--panel); padding: 0 18px; }
  .composer input:focus { outline: none; border-color: #4a4a4a; }
  .sendbtn { width: 46px; height: 46px; border-radius: 23px; background: var(--selection); color: var(--text3); display: flex; align-items: center; justify-content: center; }
  .sendbtn.ready { background: var(--text); color: var(--on-light); }
</style>
