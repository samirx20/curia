<script lang="ts">
  import { icons } from "./icons";
  let { name = "", path = "", size = 18, stroke = 1.8, color = "currentColor", filled = false } = $props();
  const d = $derived(path || icons[name] || "");
</script>

<svg width={size} height={size} viewBox="0 0 24 24" aria-hidden="true" fill={filled ? color : "none"} stroke={filled ? "none" : color}
  stroke-width={stroke} stroke-linecap="round" stroke-linejoin="round"><path {d} /></svg>

<style>
  svg { flex: none; display: block; }
</style>
