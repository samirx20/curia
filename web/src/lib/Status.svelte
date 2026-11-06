<script lang="ts">
  // A thread's or a step's state: a turning arc while running, a check when done, an amber
  // dot when it needs you, a red cross when it failed.
  import Icon from "./Icon.svelte";
  let { state = "done", size = 16 }: { state?: string; size?: number } = $props();
</script>

{#if state === "running"}
  <span class="spin" style:width="{size}px" style:height="{size}px">
    <Icon name="ring" {size} stroke={2.4} color="var(--track)" />
    <span class="arc"><Icon name="arc" {size} stroke={2.4} color="var(--text2)" /></span>
  </span>
{:else if state === "needs"}
  <Icon name="dot" {size} filled color="var(--amber)" />
{:else if state === "failed"}
  <Icon name="close" {size} stroke={2.4} color="var(--red)" />
{:else if state === "paused"}
  <Icon name="pause" {size} stroke={2.4} color="var(--text3)" />
{:else}
  <Icon name="check" {size} stroke={2.4} color="var(--text3)" />
{/if}

<style>
  .spin { position: relative; display: block; flex: none; }
  .arc { position: absolute; inset: 0; animation: turn 0.9s linear infinite; }
  @keyframes turn { to { transform: rotate(360deg); } }
</style>
