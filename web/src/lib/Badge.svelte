<script lang="ts">
  // A team's icon: its CLI's mark on a small tile, with the team's two letters.
  import Icon from "./Icon.svelte";
  import { clis, initials } from "./icons";
  let { team, size = 22 }: { team: { cli: string; name: string } | null | undefined; size?: number } = $props();
</script>

{#if team}
  <span class="badge" style:width="{size}px" style:height="{size}px" title={team.name}>
    <Icon path={clis[team.cli]?.glyph ?? ""} size={Math.round(size * 0.62)} stroke={2.2} color="#C8C8C8" />
    {#if team.name}<span class="tag">{initials(team.name)}</span>{/if}
  </span>
{/if}

<style>
  .badge { position: relative; flex: none; display: inline-flex; align-items: center; justify-content: center; border-radius: 6px; background: var(--line); }
  .tag { position: absolute; right: -6px; bottom: -5px; height: 12px; padding: 0 3px; border-radius: 3px; background: #d9d9d9; color: var(--on-light); font-size: 8.5px; font-weight: 700; line-height: 12px; letter-spacing: 0.2px; }
</style>
