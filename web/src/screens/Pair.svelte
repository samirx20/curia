<script lang="ts">
  // Pairing (Tech Stack §5): the address carries a one-time code (in its fragment, which the
  // browser never sends anywhere). This device makes its own key, gives the host the public
  // half with the code, and from then on signs in with the key.
  import Icon from "../lib/Icon.svelte";
  import { link } from "../lib/link";
  import { guessName, newKeys, remember } from "../lib/device";

  let { code }: { code: string } = $props();
  let name = $state(guessName());
  let busy = $state(false);
  let error = $state("");

  async function pair() {
    busy = true;
    error = "";
    try {
      await link.bare();
      const { keys, publicKey } = await newKeys();
      const r = await link.call<{ device_id: string; role: string }>("pair", { code, name: name.trim(), public_key: publicKey });
      await remember({ deviceId: r.device_id, role: r.role, keys });
      history.replaceState(null, "", location.pathname); // the code is used up; drop it from the address
      dispatchEvent(new HashChangeEvent("hashchange"));
      link.reconnect();
      await link.start();
    } catch (e) {
      error = (e as Error).message;
    } finally {
      busy = false;
    }
  }
</script>

<div class="screen pair">
  <div class="mark"><img src="/icon-192.png" alt="" width="72" height="72" /></div>
  {#if code}
    <h1>Pair this device</h1>
    <p class="muted">It will follow your runs and answer approvals. Its key stays on this device; the host keeps only the public half.</p>
    <label>
      <span class="muted">Name</span>
      <input bind:value={name} maxlength="40" />
    </label>
    <button class="btn primary" disabled={busy || !name.trim()} onclick={pair}>{busy ? "Pairing…" : "Pair"}</button>
    {#if error}<p class="error"><Icon name="warning" size={16} color="var(--red)" />{error}</p>{/if}
  {:else}
    <h1>Not paired</h1>
    <p class="muted">This device isn't paired with the host, or the host forgot it. On the host, run <code>curia pair</code> and open the address it prints here.</p>
  {/if}
</div>

<style>
  .pair { justify-content: center; padding: 24px; gap: 14px; max-width: 420px; margin: 0 auto; }
  .mark img { border-radius: 18px; display: block; }
  h1 { margin: 8px 0 0; font-size: 26px; letter-spacing: -0.3px; }
  p { margin: 0; }
  label { display: flex; flex-direction: column; gap: 6px; margin-top: 8px; }
  input { height: 46px; border-radius: var(--radius-small); border: 1px solid var(--line); background: var(--field); padding: 0 14px; }
  input:focus { outline: none; border-color: #4a4a4a; }
  .btn { margin-top: 4px; }
  .error { display: flex; gap: 8px; align-items: center; color: var(--red); }
  code { background: var(--raised); padding: 2px 6px; border-radius: 6px; }
</style>
