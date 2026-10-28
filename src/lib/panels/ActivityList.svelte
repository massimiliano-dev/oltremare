<script lang="ts">
  // Attività recenti di tutti gli agenti, e il registro delle azioni sul computer.
  import { onMount } from "svelte";
  import Icon from "../Icon.svelte";
  import { api, type AuditEntry, type Run } from "../api";
  import { agents } from "../state/agents.svelte";
  import { runs } from "../state/runs.svelte";
  import { listDate, time } from "../format";
  import { toolIcon } from "../run/toolmeta";

  let view = $state<"runs" | "audit">("runs");
  let list = $state<Run[]>([]);
  let audit = $state<AuditEntry[]>([]);
  let more = $state(true);
  let loading = $state(false);

  async function load(reset = false) {
    if (loading) return;
    loading = true;
    try {
      if (view === "runs") {
        const page = await api.runs(null, 40, reset ? 0 : list.length);
        list = reset ? page : [...list, ...page];
        more = page.length === 40;
      } else {
        const page = await api.audit(80, reset ? 0 : audit.length);
        audit = reset ? page : [...audit, ...page];
        more = page.length === 80;
      }
    } finally {
      loading = false;
    }
  }

  async function open(r: Run) {
    if (agents.selected?.id !== r.agentId) agents.select(r.agentId);
    await runs.openConversation(r.conversationId);
    runs.selectedRunId = r.id;
  }

  onMount(() => load(true));
  $effect(() => {
    void view;
    load(true);
  });
  // Si aggiorna quando un'esecuzione finisce
  $effect(() => {
    void runs.active;
    if (!runs.active) load(true);
  });

  const DOT: Record<string, string> = { done: "ok", error: "bad", refused: "bad", cancelled: "muted", running: "live", awaitingApproval: "warn" };
</script>

<div class="seg2" role="radiogroup">
  <button role="radio" aria-checked={view === "runs"} class:on={view === "runs"} onclick={() => (view = "runs")}>Esecuzioni</button>
  <button role="radio" aria-checked={view === "audit"} class:on={view === "audit"} onclick={() => (view = "audit")}>Azioni sul computer</button>
</div>
<div class="list">
  {#if view === "runs"}
    {#each list as r (r.id)}
      {@const a = agents.byId(r.agentId)}
      <button class="item" onclick={() => open(r)}>
        <span class="dot {DOT[r.status] ?? 'muted'}"></span>
        <span class="txt"><strong>{r.title}</strong><small>{a?.name ?? "Agente eliminato"} · {r.trigger === "main" ? "chat" : r.trigger}</small></span>
        <small class="when">{listDate(r.startedAt)}</small>
      </button>
    {:else}
      <p class="empty">{loading ? "Carico…" : "Ancora nessuna esecuzione."}</p>
    {/each}
  {:else}
    {#each audit as e, i (i)}
      <div class="item static">
        <span class="ico" class:bad={!e.ok}><Icon name={toolIcon(undefined, e.tool)} size={13} /></span>
        <span class="txt">
          <strong>{e.tool.replaceAll("_", " ")}</strong>
          <small>{agents.byId(e.agentId)?.name ?? "—"} · {e.decision === "deny" ? "negato" : e.by === "user" ? "approvato da te" : "consentito"}{e.error ? ` · ${e.error}` : ""}</small>
        </span>
        <small class="when">{time(e.at)}</small>
      </div>
    {:else}
      <p class="empty">{loading ? "Carico…" : "Nessuna azione sul computer finora."}</p>
    {/each}
  {/if}
  {#if more && (view === "runs" ? list.length : audit.length)}
    <button class="link more" onclick={() => load()}>Mostra altre</button>
  {/if}
</div>

<style>
  .seg2 {
    display: flex;
    gap: 4px;
    margin-bottom: 8px;
  }
  .seg2 button {
    height: 26px;
    padding: 0 10px;
    border-radius: 999px;
    font-size: var(--t-xs);
    font-weight: 600;
    color: var(--ink-3);
    box-shadow: inset 0 0 0 1px var(--rule);
  }
  .seg2 button.on {
    color: var(--accent);
    box-shadow: inset 0 0 0 1.5px var(--accent);
  }
  .list {
    flex: 1;
    overflow-y: auto;
    display: grid;
    align-content: start;
    gap: 1px;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 7px 6px;
    border-radius: var(--r-sm);
    text-align: left;
    min-width: 0;
  }
  button.item:hover {
    background: var(--paper-2);
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex: none;
    background: var(--ink-3);
  }
  .dot.ok {
    background: var(--ok);
  }
  .dot.bad {
    background: var(--danger);
  }
  .dot.warn {
    background: var(--warn);
  }
  .dot.live {
    background: var(--accent);
  }
  .ico {
    color: var(--ink-3);
    display: grid;
    flex: none;
  }
  .ico.bad {
    color: var(--danger);
  }
  .txt {
    flex: 1;
    min-width: 0;
    display: grid;
  }
  .txt strong {
    font-size: var(--t-sm);
    font-weight: 550;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .txt small {
    font-size: 0.7rem;
    color: var(--ink-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .when {
    font-size: 0.7rem;
    color: var(--ink-3);
    flex: none;
  }
  .empty {
    font-size: var(--t-sm);
    color: var(--ink-3);
  }
  .more {
    justify-self: center;
    margin: 8px 0;
    font-size: var(--t-xs);
  }
</style>
