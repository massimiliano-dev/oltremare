<script lang="ts">
  // Richiesta di approvazione: cosa vuole fare l'agente, e i pulsanti per decidere.
  // Invio approva, Esc nega, ⇧Invio consente per tutta l'esecuzione. Per le azioni ad alto
  // rischio serve ⌘Invio: niente approvazioni di riflesso.
  import Icon from "../Icon.svelte";
  import Diff from "./Diff.svelte";
  import { approvals } from "../state/approvals.svelte";
  import { categoryMeta, RISK, toolIcon } from "./toolmeta";
  import { mcp } from "../state/mcp.svelte";
  import type { ApprovalRequest } from "../api";

  let { request, compact = false }: { request: ApprovalRequest; compact?: boolean } = $props();
  let reason = $state("");
  let askReason = $state(false);

  const left = $derived(approvals.remaining(request));
  const seconds = $derived(Math.ceil(left / 1000));
  const fraction = $derived(Math.min(1, left / Math.max(left, totalFor(request))));
  const danger = $derived(request.risk === "danger");
  const input = $derived(request.input ?? {});

  function totalFor(r: ApprovalRequest) {
    return seenTotals.get(r.id) ?? (seenTotals.set(r.id, Math.max(1000, r.expiresAt - Date.now())), seenTotals.get(r.id)!);
  }

  function str(k: string) {
    const v = input[k];
    return typeof v === "string" ? v : v == null ? "" : JSON.stringify(v);
  }
</script>

<script lang="ts" module>
  const seenTotals = new Map<string, number>();
</script>

<div class="card r-{request.risk}" class:compact role="alertdialog" aria-label="Richiesta di approvazione: {request.title}">
  <div class="head">
    <span class="ico"><Icon name={toolIcon(request.category, request.tool)} size={16} /></span>
    <div class="what">
      <strong>{request.title}</strong>
      <small>{request.agentName} · {categoryMeta(request.category, (id) => mcp.name(id)).label} · {RISK[request.risk]}</small>
    </div>
    <svg class="ring" class:low={seconds <= 10} viewBox="0 0 36 36" aria-label="{seconds} secondi">
      <circle cx="18" cy="18" r="15" class="track" />
      <circle cx="18" cy="18" r="15" class="bar" style:stroke-dashoffset={94.25 * (1 - fraction)} />
      <text x="18" y="22" text-anchor="middle">{seconds}</text>
    </svg>
  </div>

  {#if !compact}
    <div class="body">
      {#if request.tool === "esegui_comando"}
        <pre class="cmd">$ {str("comando")}</pre>
        {#if str("cartella")}<p class="meta">in {str("cartella")}</p>{/if}
      {:else if request.diff}
        <p class="sum">{request.summary}</p>
        <Diff diff={request.diff} max={18} />
      {:else if request.tool === "apri_url"}
        <p class="sum url">{str("url")}</p>
      {:else if request.summary && request.summary !== request.title}
        <p class="sum">{request.summary}</p>
      {/if}
    </div>
  {/if}

  {#if askReason}
    <input
      class="input reason"
      bind:value={reason}
      placeholder="Perché no? (lo dico all'agente)"
      onkeydown={(e) => {
        if (e.key === "Enter") approvals.answer(request, "deny", reason);
        if (e.key === "Escape") askReason = false;
      }}
    />
  {/if}

  <div class="actions">
    <button class="btn btn-primary" onclick={() => approvals.answer(request, "allow")}>
      Approva <kbd>{danger ? "⌘↵" : "↵"}</kbd>
    </button>
    <button class="btn btn-line" onclick={() => (askReason ? approvals.answer(request, "deny", reason) : approvals.answer(request, "deny"))}>
      Nega <kbd>esc</kbd>
    </button>
    <button class="btn btn-line" onclick={() => approvals.answer(request, "allowAlways")} title="Salva la regola «Consenti» per {request.agentName}: questa azione non chiederà più">
      Consenti sempre <kbd>⌥↵</kbd>
    </button>
    {#if request.allowSession && !compact}
      <button class="btn btn-quiet" onclick={() => approvals.answer(request, "allowSession")}>Solo per questa richiesta <kbd>⇧↵</kbd></button>
    {/if}
    {#if !askReason && !compact}
      <button class="link why" onclick={() => (askReason = true)}>Nega con un motivo</button>
    {/if}
  </div>
</div>

<style>
  .card {
    display: grid;
    gap: 10px;
    padding: 14px 14px 12px;
    border-radius: var(--r-lg);
    background: var(--paper);
    box-shadow: 0 0 0 1.5px var(--accent-wash), 0 10px 30px -16px oklch(0.3 0.12 var(--accent-h) / 0.35);
    animation: settle 0.3s var(--ease-out);
  }
  .card.r-danger {
    box-shadow: 0 0 0 1.5px oklch(0.6 0.17 25 / 0.45), 0 10px 30px -16px oklch(0.4 0.15 25 / 0.35);
  }
  .card.r-write {
    box-shadow: 0 0 0 1.5px oklch(0.72 0.15 75 / 0.5), 0 10px 30px -16px oklch(0.4 0.1 75 / 0.3);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .ico {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: 10px;
    flex: none;
    color: var(--accent);
    background: var(--accent-mist);
  }
  .r-write .ico {
    color: var(--warn-ink);
    background: oklch(0.72 0.15 75 / 0.15);
  }
  .r-danger .ico {
    color: var(--danger);
    background: var(--danger-wash);
  }
  .what {
    flex: 1;
    min-width: 0;
    display: grid;
  }
  .what strong {
    font-size: var(--t-md);
    font-weight: 650;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .what small {
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
  .ring {
    width: 34px;
    height: 34px;
    flex: none;
  }
  .ring circle {
    fill: none;
    stroke-width: 3;
  }
  .track {
    stroke: var(--rule);
  }
  .bar {
    stroke: var(--accent);
    stroke-linecap: round;
    stroke-dasharray: 94.25;
    transform: rotate(-90deg);
    transform-origin: 18px 18px;
    transition: stroke-dashoffset 0.25s linear;
  }
  .ring.low .bar {
    stroke: var(--danger);
  }
  .ring text {
    font-size: 11px;
    font-weight: 700;
    fill: var(--ink-2);
    font-variant-numeric: tabular-nums;
  }
  .body {
    display: grid;
    gap: 6px;
    min-width: 0;
  }
  .cmd {
    margin: 0;
    padding: 10px 12px;
    border-radius: var(--r-md);
    background: oklch(0.2 0.04 var(--tint-h));
    color: oklch(0.92 0.03 var(--tint-h));
    font-family: var(--font-mono);
    font-size: 0.8rem;
    white-space: pre-wrap;
    word-break: break-word;
    user-select: text;
    -webkit-user-select: text;
  }
  .sum {
    margin: 0;
    font-size: var(--t-sm);
    color: var(--ink-2);
    overflow-wrap: anywhere;
  }
  .url {
    font-family: var(--font-mono);
    font-size: 0.8rem;
  }
  .meta {
    margin: 0;
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
  .reason {
    height: 34px;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .actions kbd {
    background: oklch(1 0 0 / 0.16);
    color: inherit;
    box-shadow: none;
  }
  .btn-line kbd,
  .btn-quiet kbd {
    background: var(--paper-2);
    color: var(--ink-3);
  }
  .why {
    margin-left: auto;
    font-size: var(--t-xs);
  }
  .compact .actions .btn {
    height: 28px;
  }
  @media (prefers-reduced-motion: reduce) {
    .bar {
      transition: none;
    }
  }
</style>
