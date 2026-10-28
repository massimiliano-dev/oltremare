<script lang="ts">
  import Icon from "../Icon.svelte";
  import { markdown } from "../markdown";
  import { duration, money, time, tokens } from "../format";
  import { runs } from "../state/runs.svelte";
  import type { RunStatus, Step } from "../api";
  import ToolCard from "../run/ToolCard.svelte";
  import ActivityList from "./ActivityList.svelte";
  import NotificationsList from "./NotificationsList.svelte";
  import { notifications } from "../state/notifications.svelte";
  import { on } from "../events";
  import { onMount } from "svelte";

  let tab = $state<"steps" | "activity" | "notifications">("steps");
  onMount(() => {
    const u = on("open-run", () => (tab = "steps"));
    return () => u.then((f) => f());
  });

  let expanded = $state<Record<string, boolean>>({});
  const run = $derived(runs.selected);

  const STATUS: Record<RunStatus, string> = {
    running: "In corso",
    awaitingApproval: "In attesa di te",
    done: "Completata",
    error: "Errore",
    cancelled: "Interrotta",
    refused: "Rifiutata",
  };
  const ICON: Record<string, string> = { user: "user", thinking: "bulb", text: "chat", notice: "info", tool: "bolt" };
  const LABEL: Record<string, string> = { user: "Richiesta", thinking: "Ragionamento", text: "Risposta", notice: "Nota", tool: "Strumento" };

  let now = $state(Date.now());
  $effect(() => {
    if (run?.status !== "running") return;
    const t = setInterval(() => (now = Date.now()), 250);
    return () => clearInterval(t);
  });

  const elapsed = $derived(run ? (run.endedAt ?? now) - run.startedAt : 0);
  const long = (s: Step) => s.text.length > 280;
</script>

<section class="panel" aria-label="Passaggi">
  <div class="head">
    <div class="tabs" role="tablist">
      <button role="tab" aria-selected={tab === "steps"} class:on={tab === "steps"} onclick={() => (tab = "steps")}>Passaggi</button>
      <button role="tab" aria-selected={tab === "activity"} class:on={tab === "activity"} onclick={() => (tab = "activity")}>Attività</button>
      <button role="tab" aria-selected={tab === "notifications"} class:on={tab === "notifications"} onclick={() => (tab = "notifications")}>
        Notifiche{#if notifications.unread}<span class="count">{notifications.unread > 99 ? "99+" : notifications.unread}</span>{/if}
      </button>
    </div>
    {#if run && tab === "steps"}
      <span class="chip st-{run.status}">{#if run.status === "running"}<span class="pulse"></span>{/if}{STATUS[run.status]}</span>
    {/if}
  </div>

  {#if tab === "activity"}
    <ActivityList />
  {:else if tab === "notifications"}
    <NotificationsList />
  {:else if !run}
    <p class="empty">Qui vedrai cosa fa l'agente, passo dopo passo: ragionamenti, risposte e, presto, le azioni sul computer.</p>
  {:else}
    <ol class="steps">
      {#each run.steps as s (s.id)}
        {#if s.kind === "tool"}
          <li class="step k-tool"><span class="node"><Icon name="bolt" size={13} /></span><div class="card toolwrap"><ToolCard step={s} /></div></li>
        {:else}
        <li class="step k-{s.kind}">
          <span class="node"><Icon name={ICON[s.kind] ?? "spark"} size={13} /></span>
          <div class="card">
            <div class="top">
              <strong>{LABEL[s.kind] ?? s.kind}</strong>
              <small>{time(s.at)}{s.endedAt && s.kind !== "user" ? ` · ${duration(s.endedAt - s.at)}` : ""}</small>
            </div>
            {#if s.attachments?.length}
              <p class="files">{s.attachments.map((f) => f.name).join(" · ")}</p>
            {/if}
            {#if s.text}
              <div class="md txt" class:clamp={long(s) && !expanded[s.id]}>{@html markdown(s.text)}</div>
              {#if long(s)}
                <button class="more" onclick={() => (expanded[s.id] = !expanded[s.id])}>{expanded[s.id] ? "Meno" : "Mostra tutto"}</button>
              {/if}
            {/if}
          </div>
        </li>
        {/if}
      {/each}
      {#if run.status === "error" && run.error}
        <li class="step k-error">
          <span class="node"><Icon name="alert" size={13} /></span>
          <div class="card"><div class="top"><strong>Errore</strong></div><p class="errtxt">{run.error}</p></div>
        </li>
      {/if}
    </ol>

    <dl class="stats">
      <div><dt>Durata</dt><dd>{duration(elapsed)}</dd></div>
      {#if run.usage}
        <div><dt>Token</dt><dd>{tokens(run.usage.inputTokens + run.usage.cacheRead + run.usage.cacheWrite)} → {tokens(run.usage.outputTokens)}</dd></div>
        {#if run.usage.cacheRead}<div><dt>Dalla cache</dt><dd>{tokens(run.usage.cacheRead)}</dd></div>{/if}
      {/if}
      {#if run.costEstimate != null}<div><dt>Costo stimato</dt><dd>{money(run.costEstimate)}</dd></div>{/if}
      {#if run.servedModel}<div><dt>Modello</dt><dd class="mono">{run.servedModel}</dd></div>{/if}
    </dl>
  {/if}
</section>

<style>
  .panel {
    display: flex;
    flex-direction: column;
    min-height: 0;
    flex: 1;
    padding: 12px 16px 14px;
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 10px;
  }
  .tabs {
    display: flex;
    gap: 2px;
  }
  .tabs button {
    height: 24px;
    padding: 0 9px;
    border-radius: 6px;
    font-size: var(--t-xs);
    font-weight: 650;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--ink-3);
  }
  .tabs button.on {
    color: var(--accent);
    background: var(--accent-mist);
  }
  .count {
    margin-left: 5px;
    padding: 1px 5px;
    border-radius: 999px;
    font-size: 0.62rem;
    letter-spacing: 0;
    color: white;
    background: var(--accent);
  }
  .toolwrap {
    padding-top: 0;
  }
  .k-tool .node {
    color: var(--accent);
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 22px;
    padding: 0 9px;
    border-radius: 999px;
    font-size: 0.7rem;
    font-weight: 650;
    color: var(--ink-2);
    background: var(--paper-3);
  }
  .st-running {
    color: var(--accent);
    background: var(--accent-mist);
  }
  .st-done {
    color: var(--ok);
  }
  .st-error,
  .st-refused {
    color: var(--danger);
    background: var(--danger-wash);
  }
  .st-awaitingApproval {
    color: var(--warn-ink);
  }
  .pulse {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: currentColor;
    animation: p 1s ease-in-out infinite;
  }
  @keyframes p {
    50% {
      opacity: 0.3;
    }
  }
  .empty {
    margin: 6px 0;
    font-size: var(--t-sm);
    line-height: 1.55;
    color: var(--ink-3);
  }
  .steps {
    list-style: none;
    margin: 0;
    padding: 0;
    flex: 1;
    overflow-y: auto;
    position: relative;
  }
  .step {
    position: relative;
    display: grid;
    grid-template-columns: 24px 1fr;
    gap: 10px;
    padding-bottom: 12px;
    animation: settle 0.3s var(--ease-out);
  }
  /* Il filo della timeline */
  .step::before {
    content: "";
    position: absolute;
    left: 11.5px;
    top: 24px;
    bottom: 0;
    width: 1px;
    background: var(--rule);
  }
  .step:last-child::before {
    display: none;
  }
  .node {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    color: var(--ink-3);
    background: var(--paper-2);
    box-shadow: inset 0 0 0 1px var(--rule);
  }
  .k-text .node {
    color: var(--accent);
    background: var(--accent-mist);
    box-shadow: inset 0 0 0 1px var(--accent-wash);
  }
  .k-user .node {
    color: var(--ink);
  }
  .k-error .node {
    color: var(--danger);
    background: var(--danger-wash);
    box-shadow: none;
  }
  .card {
    min-width: 0;
    padding-top: 3px;
  }
  .top {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 8px;
  }
  .top strong {
    font-size: var(--t-sm);
    font-weight: 600;
  }
  .top small {
    font-size: 0.7rem;
    color: var(--ink-3);
    white-space: nowrap;
  }
  .txt {
    margin-top: 4px;
    font-size: var(--t-sm);
    line-height: 1.5;
    color: var(--ink-2);
  }
  .files {
    margin: 4px 0 0;
    font-size: var(--t-xs);
    color: var(--accent);
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .k-thinking .txt {
    color: var(--ink-3);
    font-style: italic;
  }
  .clamp {
    max-height: 7.5em;
    overflow: hidden;
    mask-image: linear-gradient(#000 60%, transparent);
  }
  .more {
    margin-top: 2px;
    font-size: var(--t-xs);
    font-weight: 600;
    color: var(--accent);
  }
  .errtxt {
    margin: 4px 0 0;
    font-size: var(--t-sm);
    color: var(--danger);
    user-select: text;
    -webkit-user-select: text;
  }
  .stats {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px 12px;
    margin: 8px 0 0;
    padding-top: 12px;
    border-top: 1px solid var(--rule-soft);
  }
  .stats div {
    min-width: 0;
  }
  dt {
    font-size: 0.68rem;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--ink-3);
  }
  dd {
    margin: 2px 0 0;
    font-size: var(--t-sm);
    font-variant-numeric: tabular-nums;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .mono {
    font-family: var(--font-mono);
    font-size: 0.75rem;
  }
  @media (prefers-reduced-motion: reduce) {
    .pulse {
      animation: none;
    }
  }
</style>
