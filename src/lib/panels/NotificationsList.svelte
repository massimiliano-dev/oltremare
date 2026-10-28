<script lang="ts">
  import { onMount } from "svelte";
  import Icon from "../Icon.svelte";
  import { listDate } from "../format";
  import { notifications, SOURCE_LABEL } from "../state/notifications.svelte";
  import { runs } from "../state/runs.svelte";
  import { ui } from "../state/ui.svelte";
  import { settings } from "../state/settings.svelte";

  let open = $state<Record<string, boolean>>({});
  let filter = $state("");
  const today = new Date().toISOString().slice(0, 10);
  const b = $derived(notifications.briefing);
  const s = $derived(settings.s);
  const noSources = $derived(!!s && !s.notificationsSystem && !s.notificationsTeams && !s.notificationsSlack && !s.notificationsMail);
  const groups = $derived(
    filter.trim()
      ? notifications.groups
          .map(([app, l]) => [app, l.filter((n) => `${n.app} ${n.title} ${n.body}`.toLowerCase().includes(filter.trim().toLowerCase()))] as const)
          .filter(([, l]) => l.length)
      : notifications.groups,
  );

  onMount(() => notifications.refresh());

  async function startBriefing() {
    const run = await notifications.briefingNow();
    if (run) runs.openRun(run.agentId, run.conversationId);
  }
</script>

<div class="wrap">
  <div class="brief">
    <span class="sun"><Icon name="sun" size={16} /></span>
    <span class="txt">
      <strong>Briefing del mattino</strong>
      <small>
        {#if b?.date === today && b.runId}Fatto oggi{:else if b?.date === today && b.error}Non riuscito: {b.error}{:else if s?.briefingEnabled}Ogni {s.briefingDays.length === 5 && !s.briefingDays.includes(6) ? "giorno feriale" : "giorno scelto"} alle {s.briefingTime}{:else}Spento{/if}
      </small>
    </span>
    {#if b?.date === today && b.conversationId}
      <button class="btn btn-quiet sm" onclick={() => runs.openRun("briefing", b.conversationId!)}>Apri</button>
    {/if}
    <button class="btn btn-line sm" onclick={startBriefing} disabled={notifications.briefingBusy}>
      <Icon name="play" size={12} />{notifications.briefingBusy ? "Raccolgo…" : "Adesso"}
    </button>
  </div>

  <div class="bar">
    <input class="input sm" placeholder="Cerca nelle notifiche" bind:value={filter} />
    {#if notifications.unread}
      <button class="link" onclick={() => notifications.markRead()}>Segna lette ({notifications.unread})</button>
    {/if}
  </div>

  <div class="list">
    {#each groups as [app, items] (app)}
      {@const unread = items.filter((n) => !n.read).length}
      <div class="group">
        <button class="ghead" onclick={() => (open[app] = !open[app])} aria-expanded={!!open[app]}>
          <Icon name={open[app] ? "down" : "chevron"} size={12} />
          <strong>{app}</strong>
          {#if !app.toLowerCase().includes(SOURCE_LABEL[items[0].source].toLowerCase())}<small>{SOURCE_LABEL[items[0].source]}</small>{/if}
          {#if unread}<span class="badge">{unread}</span>{/if}
          <span class="when">{listDate(items[0].at)}</span>
        </button>
        {#each open[app] ? items.slice(0, 30) : items.slice(0, 1) as n (n.id)}
          <div class="n" class:read={n.read}>
            <span class="t">{n.title || app}</span>
            {#if n.body}<span class="b">{n.body}</span>{/if}
            {#if open[app]}<small class="when">{listDate(n.at)}</small>{/if}
          </div>
        {/each}
        {#if !open[app] && items.length > 1}<button class="link more" onclick={() => (open[app] = true)}>altre {items.length - 1}</button>{/if}
      </div>
    {:else}
      <p class="empty">
        {#if notifications.loading}Carico…{:else if noSources}Nessuna fonte attiva. <button class="link" onclick={() => ui.openSettings("notifications")}>Scegli da dove leggere le notifiche</button>{:else if filter}Nessuna notifica con «{filter}».{:else}Nessuna notifica nelle ultime 48 ore. Arrivano qui da sistema, Teams, Slack e posta, secondo le <button class="link" onclick={() => ui.openSettings("notifications")}>impostazioni</button>.{/if}
      </p>
    {/each}
  </div>
</div>

<style>
  .wrap {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .brief {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 10px 10px 12px;
    border-radius: var(--r-md);
    background: var(--accent-mist);
  }
  .sun {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: 50%;
    flex: none;
    color: var(--accent);
    background: var(--paper);
  }
  .txt {
    flex: 1;
    min-width: 0;
    display: grid;
  }
  .txt strong {
    font-size: var(--t-sm);
    font-weight: 650;
  }
  .txt small {
    font-size: 0.7rem;
    color: var(--ink-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sm {
    height: 28px;
    padding: 0 10px;
    font-size: var(--t-xs);
    gap: 5px;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .input.sm {
    flex: 1;
    height: 30px;
    font-size: var(--t-sm);
  }
  .bar .link {
    font-size: var(--t-xs);
    white-space: nowrap;
  }
  .list {
    flex: 1;
    overflow-y: auto;
    display: grid;
    align-content: start;
    gap: 8px;
  }
  .group {
    display: grid;
    gap: 2px;
    padding-bottom: 8px;
    border-bottom: 1px solid var(--rule-soft);
  }
  .ghead {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 2px 0;
    text-align: left;
    color: var(--ink-3);
  }
  .ghead strong {
    font-size: var(--t-sm);
    font-weight: 650;
    color: var(--ink);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ghead small {
    font-size: 0.68rem;
  }
  .badge {
    min-width: 18px;
    height: 18px;
    padding: 0 5px;
    border-radius: 999px;
    display: grid;
    place-items: center;
    font-size: 0.66rem;
    font-weight: 700;
    color: white;
    background: var(--accent);
  }
  .when {
    margin-left: auto;
    font-size: 0.68rem;
    color: var(--ink-3);
    flex: none;
  }
  .n {
    display: grid;
    gap: 1px;
    padding: 4px 0 4px 18px;
    font-size: var(--t-sm);
  }
  .n.read {
    opacity: 0.62;
  }
  .t {
    font-weight: 550;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .b {
    color: var(--ink-2);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    line-height: 1.4;
  }
  .n .when {
    margin-left: 0;
  }
  .more {
    justify-self: start;
    margin-left: 18px;
    font-size: var(--t-xs);
  }
  .empty {
    font-size: var(--t-sm);
    color: var(--ink-3);
    line-height: 1.5;
  }
</style>
