<script lang="ts">
  // Runs grouped by project folder, each run's team threads under it (docs: Design 30).
  import Icon from "../lib/Icon.svelte";
  import Badge from "../lib/Badge.svelte";
  import Status from "../lib/Status.svelte";
  import { ago } from "../lib/icons";
  import { app, needsYou, teamOf } from "../lib/state.svelte";

  const folderOf = (cwd: string) => cwd.split(/[\\/]/).filter(Boolean).pop() ?? cwd;
  const groups = $derived.by(() => {
    const out: { folder: string; runs: any[] }[] = [];
    for (const r of [...app.threads].filter((t) => !t.parent_id).sort((a, b) => b.updated_at - a.updated_at)) {
      const f = folderOf(r.cwd);
      let g = out.find((x) => x.folder === f);
      if (!g) out.push((g = { folder: f, runs: [] }));
      g.runs.push(r);
    }
    return out;
  });
  const teamThreads = (rootId: string) => app.threads.filter((t) => t.root_id === rootId && t.id !== rootId);
  const state = (t: any) => (needsYou(t.id) ? "needs" : t.status === "running" ? "running" : "done");
  const working = $derived(app.threads.filter((t) => t.status === "running").length);
  const waiting = $derived(new Set(app.inbox.map((q) => q.thread_id)).size);
  function note(r: any): string {
    if (needsYou(r.id)) return "Needs you";
    const n = teamThreads(r.id).length;
    if (r.status === "running") return ago(r.updated_at);
    return n ? `${n} ${n === 1 ? "team" : "teams"}` : "Done";
  }
</script>

<div class="scroll">
  <h1 class="title">Runs</h1>
  <div class="chips">
    <span class="chip"><Status state={working ? "running" : "done"} size={14} />{working} working</span>
    {#if waiting}<a class="chip" href="#/inbox"><Status state="needs" size={14} />{waiting} need you</a>{/if}
  </div>

  {#each groups as g (g.folder)}
    <div class="folder"><Icon name="folder" size={15} />{g.folder}</div>
    {#each g.runs as r (r.id)}
      {@const team = teamThreads(r.id)}
      <div class="card run">
        <a class="row main" href="#/t/{r.id}">
          <Status state={state(r)} />
          <span class="name">{r.title}</span>
          <span class="note" class:amber={needsYou(r.id)}>{note(r)}</span>
          <Badge team={teamOf(r)} />
          <Icon name="open" size={16} color="var(--text3)" />
        </a>
        {#if team.length}
          <div class="team">
            {#each team as t (t.id)}
              <a class="row" href="#/t/{t.id}">
                <Status state={state(t)} size={14} />
                <span class="name sub" class:dim={state(t) === "done"}>{t.title}</span>
                {#if needsYou(t.id)}<span class="note amber">Needs you</span>{/if}
                <Badge team={teamOf(t)} />
              </a>
            {/each}
          </div>
        {/if}
      </div>
    {/each}
  {:else}
    <div class="empty">
      <Icon name="runs" size={32} stroke={1.5} color="var(--text3)" />
      <p>No runs yet.</p>
      <p class="muted">Start one on the host; it shows here at once.</p>
    </div>
  {/each}
</div>

<style>
  .chips { display: flex; gap: 8px; padding: 0 8px 4px; }
  a.chip { text-decoration: none; }
  .run { margin-bottom: 10px; overflow: hidden; }
  .row { display: flex; align-items: center; gap: 12px; padding: 14px 16px; color: var(--text); text-decoration: none; min-height: 52px; }
  .row:active { background: var(--raised); }
  .main .name { font-weight: 600; }
  .name { flex: 1; min-width: 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .sub { font-size: 15px; }
  .dim { color: var(--text2); }
  .note { color: var(--text3); font-size: 14px; }
  .amber { color: var(--amber); }
  .team { margin: -4px 0 8px 22px; border-left: 1px solid var(--line); }
  .team .row { padding: 10px 16px 10px 16px; min-height: 44px; }
  .empty { display: flex; flex-direction: column; align-items: center; gap: 4px; margin-top: 80px; text-align: center; }
  .empty p { margin: 0; }
</style>
