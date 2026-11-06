<script lang="ts">
  // The host's teams and how much of each account's allowance is left.
  import Badge from "../lib/Badge.svelte";
  import { cliLabel } from "../lib/icons";
  import { app } from "../lib/state.svelte";

  const known = (v: unknown) => v !== null && v !== undefined;
  const resets = (ms: number | null) => (ms ? ` · resets ${new Date(ms).toLocaleString(undefined, { weekday: "short", hour: "2-digit", minute: "2-digit" })}` : "");
</script>

<div class="scroll">
  <h1 class="title">Teams</h1>
  {#each app.teams as t (t.id)}
    <div class="card team">
      <div class="head">
        <Badge team={t} size={26} />
        <div class="names"><b>{t.name}</b><span class="muted">{cliLabel(t.cli)}{t.model ? ` · ${t.model}` : ""}</span></div>
        {#if t.out_of_quota_until_ms}<span class="amber">Out of quota</span>{/if}
      </div>
      {#each [["5-hour", t.five_hour_percent_left, t.five_hour_resets_at_ms], ["Week", t.weekly_percent_left, t.weekly_resets_at_ms]] as [label, left, at]}
        <div class="meter">
          <div class="line"><span>{label}</span><span class="muted">{known(left) ? `${Math.round(left as number)}% left${resets(at as number)}` : "not reported"}</span></div>
          <div class="bar"><div class:low={known(left) && (left as number) < 15} style:width="{known(left) ? 100 - (left as number) : 0}%"></div></div>
        </div>
      {/each}
    </div>
  {:else}
    <p class="muted empty">No teams yet. Add one on the host.</p>
  {/each}
</div>

<style>
  .team { padding: 14px 16px; display: flex; flex-direction: column; gap: 12px; margin-bottom: 10px; }
  .head { display: flex; align-items: center; gap: 14px; }
  .names { flex: 1; display: flex; flex-direction: column; font-size: 14px; }
  .names b { font-size: 16px; }
  .amber { color: var(--amber); font-size: 13px; }
  .meter { display: flex; flex-direction: column; gap: 6px; }
  .line { display: flex; justify-content: space-between; font-size: 14px; }
  .bar { height: 4px; border-radius: 2px; background: var(--selection); overflow: hidden; }
  .bar div { height: 100%; background: var(--text2); }
  .bar div.low { background: var(--amber); }
  .empty { text-align: center; margin-top: 80px; }
</style>
