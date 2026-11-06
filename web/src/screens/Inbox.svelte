<script lang="ts">
  // Everything waiting on you, across every run (docs: Design 32). Answers are signed with
  // this device's key.
  import Icon from "../lib/Icon.svelte";
  import Badge from "../lib/Badge.svelte";
  import Status from "../lib/Status.svelte";
  import { link } from "../lib/link";
  import { app, soon, teamOf } from "../lib/state.svelte";
  import { asked } from "../lib/rows.svelte";
  import { ago, who } from "../lib/icons";

  let replies: Record<string, string> = $state({});
  let error = $state("");
  const threadOf = (id: string) => app.threads.find((t) => t.id === id);
  const canAnswer = $derived(app.role === "owner" || app.role === "approve");

  async function answer(q: any, decision: any) {
    error = "";
    try {
      await link.answer(q.thread_id, q.request_id, decision);
      soon();
    } catch (e) {
      error = (e as Error).message;
    }
  }
  function reply(q: any) {
    const text = (replies[q.request_id] ?? "").trim();
    if (!text) return;
    const question = q.input?.questions?.[0];
    const key = question?.id ?? question?.question ?? "answer";
    answer(q, { decision: "answer", answers: { [key]: text } });
  }
</script>

<div class="scroll">
  <h1 class="title">Inbox</h1>
  <div class="chips"><span class="chip on">Needs you · {app.inbox.length}</span></div>
  {#if error}<p class="error"><Icon name="warning" size={16} color="var(--red)" />{error}</p>{/if}

  {#each app.inbox as q (q.thread_id + q.request_id)}
    {@const t = threadOf(q.thread_id)}
    <div class="card item">
      <div class="head">
        <Status state="needs" size={13} />
        <b>{q.thread_title}</b>
        <span class="muted when">{ago(q.created_at)}</span>
      </div>
      <div class="muted sub"><Badge team={teamOf(t)} size={16} />{who(teamOf(t))}{t && !t.parent_id ? " · main thread" : ""}</div>
      {#if q.kind === "question"}
        <div class="text">Asks: {q.input?.questions?.[0]?.question ?? asked(q.input)}</div>
        {#if canAnswer}
          <form class="reply" onsubmit={(e) => { e.preventDefault(); reply(q); }}>
            <input placeholder="Reply…" bind:value={replies[q.request_id]} />
            <button class="send" aria-label="Send reply"><Icon name="send" size={18} stroke={2.2} /></button>
          </form>
        {/if}
      {:else}
        <div class="text">Wants to run <span class="muted">· {q.tool_name.replace(/^mcp__([^_]+)__/, "$1: ")}</span></div>
        <div class="well">{asked(q.input)}</div>
        {#if canAnswer}
          <div class="two">
            <button class="btn primary" onclick={() => answer(q, { decision: "allow" })}>Allow</button>
            <button class="btn secondary" onclick={() => answer(q, { decision: "deny", message: null })}>Deny</button>
          </div>
        {/if}
      {/if}
      <a class="open" href="#/t/{q.thread_id}">Open thread <Icon name="open" size={14} /></a>
    </div>
  {:else}
    <div class="empty">
      <Icon name="check" size={32} stroke={1.8} color="var(--text3)" />
      <p>Nothing needs you.</p>
      <p class="muted">Approvals and questions from every run land here.</p>
    </div>
  {/each}
</div>

<style>
  .chips { display: flex; gap: 8px; padding: 0 8px 12px; }
  .chip.on { background: var(--selection); }
  .item { padding: 14px 16px; display: flex; flex-direction: column; gap: 8px; margin-bottom: 10px; }
  .head { display: flex; align-items: center; gap: 8px; }
  .head b { flex: 1; min-width: 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .when { font-size: 13px; }
  .sub { display: flex; align-items: center; gap: 10px; font-size: 13px; margin-top: -2px; }
  .text { overflow-wrap: anywhere; }
  .two { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; }
  .reply { display: flex; gap: 8px; }
  .reply input { flex: 1; height: 44px; border-radius: var(--radius-small); border: 1px solid var(--line); background: var(--well); padding: 0 14px; }
  .send { width: 44px; height: 44px; border-radius: 22px; background: var(--text); color: var(--on-light); display: flex; align-items: center; justify-content: center; }
  .open { display: flex; align-items: center; gap: 4px; align-self: flex-end; color: var(--text2); font-size: 13px; text-decoration: none; }
  .error { display: flex; gap: 8px; align-items: center; color: var(--red); padding: 0 8px; }
  .empty { display: flex; flex-direction: column; align-items: center; gap: 4px; margin-top: 80px; text-align: center; }
  .empty p { margin: 0; }
</style>
