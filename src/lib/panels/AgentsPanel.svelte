<script lang="ts">
  import Icon from "../Icon.svelte";
  import AgentDisc from "../agents/AgentDisc.svelte";
  import { agents } from "../state/agents.svelte";
  import { runs } from "../state/runs.svelte";
  import { ui } from "../state/ui.svelte";
  import { providerLabel, modelLabel } from "../state/settings.svelte";
  import { listDate } from "../format";

  const runningAgents = $derived(new Set(Object.values(runs.byId).filter((r) => r.status === "running").map((r) => r.agentId)));
</script>

<nav class="panel" aria-label="Agenti">
  <div class="head">
    <span class="eyebrow">Agenti</span>
    <button class="icon-btn" onclick={() => (ui.editing = "new")} title="Nuovo agente (⌘⇧A)" aria-label="Nuovo agente"><Icon name="plus" size={16} /></button>
  </div>
  <ul class="agents">
    {#each agents.list as a (a.id)}
      <li>
        <button class="agent" class:on={a.id === agents.selected?.id} onclick={() => agents.select(a.id)} ondblclick={() => (ui.editing = $state.snapshot(a))}>
          <AgentDisc icon={a.icon} hue={a.hue} size={30} />
          <span class="text">
            <strong>{a.name}</strong>
            <small>{providerLabel(a.provider)} · {modelLabel(a.provider, a.model)}</small>
          </span>
          {#if runningAgents.has(a.id)}<span class="live" title="Al lavoro"></span>{/if}
        </button>
      </li>
    {/each}
  </ul>

  <div class="head convs-head">
    <span class="eyebrow">Conversazioni</span>
    <button class="icon-btn" onclick={() => runs.newConversation()} title="Nuova conversazione (⌘N)" aria-label="Nuova conversazione"><Icon name="pen" size={15} /></button>
  </div>
  <ul class="convs">
    {#each runs.conversations as c (c.id)}
      <li class="conv" class:on={c.id === runs.conversationId}>
        <button class="open" onclick={() => runs.openConversation(c.id)}>
          <span class="title">{c.title}</span>
          <small>{listDate(c.updatedAt)}</small>
        </button>
        <button class="del" onclick={() => runs.deleteConversation(c)} aria-label="Elimina conversazione" title="Elimina"><Icon name="x" size={13} /></button>
      </li>
    {:else}
      <li class="empty">Le conversazioni con {agents.selected?.name ?? "l'agente"} compariranno qui.</li>
    {/each}
  </ul>
</nav>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--paper-2);
    padding: 10px 10px 14px;
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 4px 4px 6px 8px;
  }
  .convs-head {
    margin-top: 14px;
    padding-top: 12px;
    border-top: 1px solid var(--rule-soft);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .agents {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: 2px;
    max-height: 44%;
    overflow-y: auto;
  }
  .agent {
    position: relative;
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 7px 8px;
    border-radius: var(--r-md);
    text-align: left;
    transition: background 0.15s;
  }
  .agent:hover {
    background: var(--paper-3);
  }
  .agent.on {
    background: var(--paper);
    box-shadow: var(--shadow-card);
  }
  .text {
    display: grid;
    min-width: 0;
  }
  .text strong {
    font-size: var(--t-md);
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .text small {
    font-size: var(--t-xs);
    color: var(--ink-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .live {
    margin-left: auto;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--accent);
    animation: pulse 1.2s ease-in-out infinite;
  }
  @keyframes pulse {
    50% {
      opacity: 0.35;
    }
  }
  .convs {
    flex: 1;
    overflow-y: auto;
    overflow-x: hidden;
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    align-content: start;
    gap: 1px;
  }
  .conv {
    position: relative;
    display: flex;
    border-radius: var(--r-sm);
  }
  .conv:hover {
    background: var(--paper-3);
  }
  .conv.on {
    background: var(--accent-mist);
  }
  .open {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: baseline;
    gap: 8px;
    padding: 7px 8px;
    text-align: left;
  }
  .title {
    flex: 1;
    font-size: var(--t-sm);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .conv.on .title {
    color: var(--accent);
    font-weight: 600;
  }
  .open small {
    font-size: var(--t-xs);
    color: var(--ink-3);
    flex: none;
  }
  .del {
    display: none;
    place-items: center;
    width: 26px;
    color: var(--ink-3);
  }
  .conv:hover .del {
    display: grid;
  }
  .conv:hover .open small {
    display: none;
  }
  .del:hover {
    color: var(--danger);
  }
  .empty {
    padding: 8px;
    font-size: var(--t-xs);
    line-height: 1.5;
    color: var(--ink-3);
  }
  @media (prefers-reduced-motion: reduce) {
    .live {
      animation: none;
    }
  }
</style>
