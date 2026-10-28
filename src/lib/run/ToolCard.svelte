<script lang="ts">
  // Un'azione dell'agente nella timeline: riga compatta, che si apre sui dettagli.
  import Icon from "../Icon.svelte";
  import Diff from "./Diff.svelte";
  import ApprovalCard from "./ApprovalCard.svelte";
  import { approvals } from "../state/approvals.svelte";
  import { duration } from "../format";
  import { toolIcon } from "./toolmeta";
  import type { Step } from "../api";
  import { assetUrl } from "../assets";
  import { lightbox } from "../state/lightbox.svelte";
  import { runs } from "../state/runs.svelte";

  /** Tutti gli screenshot dell'esecuzione, per scorrerli nel visualizzatore */
  function openShot() {
    const run = Object.values(runs.byId).find((r) => r.steps.some((s) => s.id === step.id));
    const shots = (run?.steps ?? [])
      .filter((s) => (s.result as any)?.screenshotPath)
      .map((s) => ({ id: s.id, src: assetUrl((s.result as any).screenshotPath), caption: s.title ?? "" }));
    lightbox.show(shots, Math.max(0, shots.findIndex((x) => x.id === step.id)));
  }

  let { step }: { step: Step } = $props();
  let open = $state(false);
  let showInput = $state(false);

  const r = $derived((step.result ?? {}) as Record<string, any>);
  const pending = $derived(approvals.forStep(step.id));
  const STATUS: Record<string, string> = {
    pending: "in preparazione",
    awaitingApproval: "aspetta te",
    running: "in corso",
    done: "fatto",
    error: "errore",
    denied: "negato",
    cancelled: "fermato",
  };
</script>

<div class="tool st-{step.status}">
  <button class="row" onclick={() => (open = !open)} aria-expanded={open}>
    <span class="ico"><Icon name={toolIcon(step.category, step.tool)} size={14} /></span>
    {#if r.screenshotPath}<img class="thumb" src={assetUrl(r.screenshotPath)} alt="" loading="lazy" />{/if}
    <span class="title">{step.title ?? step.tool}</span>
    <span class="state">
      {#if step.status === "running"}<span class="spin"></span>{:else if step.status === "done"}<Icon name="check" size={13} />{:else if step.status === "error" || step.status === "denied"}<Icon name="x" size={13} />{/if}
      {STATUS[step.status ?? ""] ?? step.status}{step.endedAt && step.status === "done" ? ` · ${duration(step.endedAt - step.at)}` : ""}
    </span>
    <Icon name={open ? "down" : "chevron"} size={12} />
  </button>

  {#if pending}
    <div class="approval"><ApprovalCard request={pending} /></div>
  {/if}

  {#if open}
    <div class="detail">
      {#if step.error}<p class="err-text">{step.error}</p>{/if}
      {#if r.kind === "shell"}
        <pre class="term">$ {r.command}{#if r.stdout}{"\n"}{r.stdout}{/if}{#if r.stderr}<span class="stderr">{"\n"}{r.stderr}</span>{/if}</pre>
        <p class="meta">uscita {r.exitCode}{r.timedOut ? " · tempo scaduto" : ""}{r.truncated ? " · output troncato" : ""} · {r.cwd}</p>
      {:else if r.kind === "fs"}
        <p class="meta path">{r.from ? `${r.from} → ` : ""}{r.path}</p>
        {#if r.diff}<Diff diff={r.diff} />{/if}
        {#if r.text}<pre class="excerpt">{r.text}</pre>{/if}
        {#if r.entries?.length}
          <ul class="entries">{#each r.entries.slice(0, 40) as e, i (i)}<li>{e}</li>{/each}</ul>
          {#if r.entries.length > 40}<p class="meta">… e altre {r.entries.length - 40}</p>{/if}
        {/if}
      {:else if r.kind === "mail"}
        {#if r.to?.length}<p class="meta">A: {r.to.join(", ")}</p>{/if}
        {#if r.subject}<p class="meta"><strong>{r.subject}</strong></p>{/if}
        {#if r.preview}<pre class="excerpt">{r.preview}</pre>{/if}
      {:else if r.kind === "calendar" && r.events?.length}
        <ul class="entries">
          {#each r.events.slice(0, 20) as ev, k (k)}
            <li>{ev.start ? new Date(ev.start).toLocaleString("it-IT", { weekday: "short", day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" }) : ""} · {ev.title}</li>
          {/each}
        </ul>
      {:else if r.kind === "tasks" && r.items?.length}
        <ul class="entries">{#each r.items.slice(0, 30) as it, k (k)}<li>☐ {it.title}{it.due ? ` · ${it.due}` : ""}</li>{/each}</ul>
      {:else if r.kind === "browser"}
        {#if r.url}<p class="meta path">{r.title ? `${r.title} · ` : ""}{r.url}</p>{/if}
        {#if r.screenshotPath}<button class="shot" onclick={openShot} aria-label="Apri lo screenshot"><img src={assetUrl(r.screenshotPath)} alt="Pagina" loading="lazy" /></button>{/if}
        {#if r.text}<pre class="excerpt">{r.text}</pre>{/if}
      {:else if r.kind === "computer" && r.screenshotPath}
        <button class="shot" onclick={openShot} aria-label="Apri lo screenshot"><img src={assetUrl(r.screenshotPath)} alt="Screenshot" loading="lazy" /></button>
      {:else if r.kind === "text" && r.text && !step.error}
        <pre class="excerpt">{r.text}</pre>
      {/if}
      <button class="link small" onclick={() => (showInput = !showInput)}>{showInput ? "Nascondi i parametri" : "Parametri"}</button>
      {#if showInput}<pre class="excerpt">{JSON.stringify(step.input, null, 2)}</pre>{/if}
    </div>
  {/if}
</div>

<style>
  .tool {
    position: relative;
    border-radius: var(--r-md);
    background: var(--paper-2);
    box-shadow: inset 0 0 0 1px var(--rule-soft);
    overflow: hidden;
  }
  .tool::before {
    content: "";
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 3px;
    background: var(--rule);
  }
  .st-running::before {
    background: linear-gradient(var(--accent), var(--accent-wash), var(--accent));
    background-size: 100% 200%;
    animation: flow 1.2s linear infinite;
  }
  .st-done::before {
    background: var(--ok);
  }
  .st-error::before,
  .st-denied::before {
    background: var(--danger);
  }
  .st-awaitingApproval::before {
    background: var(--warn);
  }
  @keyframes flow {
    to {
      background-position: 0 -200%;
    }
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    min-height: 34px;
    padding: 6px 10px 6px 12px;
    text-align: left;
  }
  .ico {
    display: grid;
    place-items: center;
    color: var(--ink-3);
    flex: none;
  }
  .thumb {
    width: 34px;
    height: 22px;
    object-fit: cover;
    border-radius: 4px;
    box-shadow: 0 0 0 1px var(--rule);
    flex: none;
  }
  .shot {
    justify-self: start;
    max-width: 100%;
    border-radius: 8px;
    overflow: hidden;
    box-shadow: 0 0 0 1px var(--rule);
  }
  .shot img {
    display: block;
    max-width: 100%;
    max-height: 260px;
  }
  .title {
    flex: 1;
    min-width: 0;
    font-size: var(--t-sm);
    font-weight: 550;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .state {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: var(--t-xs);
    color: var(--ink-3);
    white-space: nowrap;
  }
  .st-done .state {
    color: var(--ok);
  }
  .st-error .state,
  .st-denied .state {
    color: var(--danger);
  }
  .st-awaitingApproval .state {
    color: var(--warn-ink);
    font-weight: 650;
  }
  .spin {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    border: 2px solid var(--accent);
    border-right-color: transparent;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .approval {
    padding: 0 10px 10px 12px;
  }
  .detail {
    display: grid;
    gap: 6px;
    padding: 0 12px 10px 14px;
    min-width: 0;
  }
  .term,
  .excerpt {
    margin: 0;
    max-height: 260px;
    overflow: auto;
    padding: 8px 10px;
    border-radius: 8px;
    font-family: var(--font-mono);
    font-size: 0.74rem;
    line-height: 1.5;
    white-space: pre-wrap;
    word-break: break-word;
    user-select: text;
    -webkit-user-select: text;
  }
  .term {
    background: oklch(0.2 0.04 var(--tint-h));
    color: oklch(0.92 0.03 var(--tint-h));
  }
  .stderr {
    color: oklch(0.78 0.12 25);
  }
  .excerpt {
    background: var(--paper);
    box-shadow: inset 0 0 0 1px var(--rule-soft);
    color: var(--ink-2);
  }
  .meta {
    margin: 0;
    font-size: var(--t-xs);
    color: var(--ink-3);
    overflow-wrap: anywhere;
  }
  .path {
    font-family: var(--font-mono);
  }
  .entries {
    margin: 0;
    padding: 6px 10px;
    list-style: none;
    max-height: 200px;
    overflow: auto;
    border-radius: 8px;
    background: var(--paper);
    font-family: var(--font-mono);
    font-size: 0.72rem;
    line-height: 1.6;
    color: var(--ink-2);
  }
  .err-text {
    margin: 0;
    font-size: var(--t-sm);
    color: var(--danger);
    user-select: text;
    -webkit-user-select: text;
  }
  .small {
    justify-self: start;
    font-size: var(--t-xs);
  }
  @media (prefers-reduced-motion: reduce) {
    .st-running::before,
    .spin {
      animation: none;
    }
  }
</style>
