<script lang="ts">
  // Screens by the address's fragment: #pair=CODE, #/t/<thread>, #/inbox, #/teams, else Runs.
  import Icon from "./lib/Icon.svelte";
  import { app } from "./lib/state.svelte";
  import Pair from "./screens/Pair.svelte";
  import Runs from "./screens/Runs.svelte";
  import Thread from "./screens/Thread.svelte";
  import Inbox from "./screens/Inbox.svelte";
  import Teams from "./screens/Teams.svelte";

  let hash = $state(location.hash);
  addEventListener("hashchange", () => (hash = location.hash));
  const pairCode = $derived(/^#pair=([A-Za-z0-9]+)/.exec(hash)?.[1] ?? "");
  const threadId = $derived(/^#\/t\/([\w-]+)/.exec(hash)?.[1] ?? "");
  const tab = $derived(hash === "#/inbox" ? "inbox" : hash === "#/teams" ? "teams" : "runs");
</script>

{#if pairCode || app.link === "unpaired"}
  <Pair code={pairCode} />
{:else}
  {#if app.link === "offline" || app.link === "connecting"}
    <div class="banner" class:offline={app.link === "offline"}>
      {app.link === "offline" ? "Can't reach the host. Trying again…" : "Connecting to the host…"}
    </div>
  {/if}
  {#if threadId}
    <Thread id={threadId} />
  {:else}
    <div class="screen">
      {#if tab === "inbox"}<Inbox />{:else if tab === "teams"}<Teams />{:else}<Runs />{/if}
      <nav>
        <a href="#/" class:on={tab === "runs"}><Icon name="runs" size={22} /><span>Runs</span></a>
        <a href="#/inbox" class:on={tab === "inbox"}>
          <span class="with-count"><Icon name="inbox" size={22} />{#if app.inbox.length}<b>{app.inbox.length}</b>{/if}</span>
          <span>Inbox</span>
        </a>
        <a href="#/teams" class:on={tab === "teams"}><Icon name="people" size={22} /><span>Teams</span></a>
      </nav>
    </div>
  {/if}
{/if}

<style>
  nav { display: flex; border-top: 1px solid var(--line); background: var(--app); padding: 8px 0 calc(8px + env(safe-area-inset-bottom)); }
  nav a { flex: 1; display: flex; flex-direction: column; align-items: center; gap: 4px; color: var(--text3); text-decoration: none; font-size: 12px; font-weight: 600; }
  nav a.on { color: var(--text); }
  .with-count { position: relative; }
  .with-count b { position: absolute; top: -6px; right: -10px; min-width: 18px; height: 18px; padding: 0 5px; border-radius: 9px; background: var(--amber); color: var(--on-light); font-size: 11px; line-height: 18px; text-align: center; }
  .banner { position: fixed; top: env(safe-area-inset-top); left: 50%; transform: translateX(-50%); margin-top: 8px; z-index: 10; padding: 6px 14px; border-radius: 16px; background: var(--raised); color: var(--text2); font-size: 13px; border: 1px solid var(--line); }
  .banner.offline { color: var(--amber); }
</style>
