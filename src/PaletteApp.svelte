<script lang="ts">
  // Palette rapida: una finestrella da qualsiasi app (⌥Spazio). Scrivi, scegli l'agente con «/»,
  // la risposta arriva qui; «Apri in Oltremare» porta alla conversazione completa.
  import { onMount, tick } from "svelte";
  import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
  import Icon from "./lib/Icon.svelte";
  import AgentDisc from "./lib/agents/AgentDisc.svelte";
  import { api, errorText, type Agent, type ApprovalRequest, type Run } from "./lib/api";
  import { on } from "./lib/events";
  import { markdown } from "./lib/markdown";
  import { settings } from "./lib/state/settings.svelte";

  const LAST = "oltremare:palette-agent";
  let agentsList = $state<Agent[]>([]);
  let agentId = $state("assistente");
  let text = $state("");
  let picking = $state(false);
  let pickIndex = $state(0);
  let run = $state<Run | null>(null);
  let answer = $state("");
  let activity = $state("");
  let status = $state<"idle" | "running" | "done" | "error">("idle");
  let error = $state("");
  let approval = $state<ApprovalRequest | null>(null);
  let input: HTMLInputElement | undefined = $state();
  let card: HTMLElement | undefined = $state();

  const agent = $derived(agentsList.find((a) => a.id === agentId) ?? agentsList[0]);
  const query = $derived(picking ? text.replace(/^\//, "").toLowerCase() : "");
  const matches = $derived(agentsList.filter((a) => a.name.toLowerCase().includes(query)));

  function reset(preselect?: string | null) {
    run = null;
    answer = "";
    activity = "";
    status = "idle";
    error = "";
    approval = null;
    text = "";
    picking = false;
    if (preselect) agentId = preselect;
    tick().then(() => input?.focus());
  }

  async function fit() {
    await tick();
    if (!card) return;
    const h = Math.min(560, Math.ceil(card.getBoundingClientRect().height) + 24);
    getCurrentWindow().setSize(new LogicalSize(720, h)).catch(() => {});
  }
  $effect(() => {
    void answer;
    void activity;
    void status;
    void approval;
    void picking;
    void matches.length;
    fit();
  });

  onMount(() => {
    document.documentElement.classList.add("palette");
    settings.load().catch(() => {});
    api.agents().then((l) => {
      agentsList = l;
      try {
        const last = localStorage.getItem(LAST);
        if (last && l.some((a) => a.id === last)) agentId = last;
      } catch {}
    });
    on("agents-changed", () => api.agents().then((l) => (agentsList = l)));
    on("palette-open", (p) => {
      if (status !== "running") reset(p.agentId);
      else if (p.agentId) agentId = p.agentId;
      input?.focus();
    });
    on("run-delta", (p) => {
      if (p.runId === run?.id && p.kind === "text") {
        answer += p.delta;
        activity = "";
      }
    });
    on("run-step", (p) => {
      if (p.runId === run?.id && p.step.kind === "tool") activity = p.step.title ?? p.step.tool ?? "";
    });
    on("run-progress", (p) => {
      if (p.runId === run?.id) activity = p.text;
    });
    on("run-approval", (p) => {
      if (p.runId === run?.id) approval = p.request;
    });
    on("run-approval-resolved", (p) => {
      if (approval?.id === p.approvalId) approval = null;
    });
    on("run-done", (p) => {
      if (p.runId !== run?.id) return;
      status = p.status === "done" ? "done" : "error";
      error = p.error ?? (p.status === "cancelled" ? "Interrotto" : "");
      activity = "";
    });
    const w = getCurrentWindow();
    const un = w.onFocusChanged(({ payload: focused }) => {
      // Fuori dalla palette si chiude; il lavoro continua e si ritrova in Oltremare
      if (!focused && !approval) api.paletteHide();
    });
    input?.focus();
    return () => {
      un.then((f) => f());
    };
  });

  async function send() {
    const t = text.trim();
    if (!t || !agent || status === "running") return;
    try {
      localStorage.setItem(LAST, agent.id);
    } catch {}
    answer = "";
    error = "";
    activity = "Ci penso…";
    status = "running";
    try {
      run = await api.runStart({ agentId: agent.id, conversationId: null, text: t, source: "palette" });
      text = "";
    } catch (e) {
      status = "error";
      error = errorText(e);
    }
  }

  function choose(a: Agent) {
    agentId = a.id;
    picking = false;
    text = "";
    input?.focus();
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      if (picking) {
        picking = false;
        text = "";
      } else api.paletteHide();
      return;
    }
    if (picking) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        pickIndex = Math.min(matches.length - 1, pickIndex + 1);
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        pickIndex = Math.max(0, pickIndex - 1);
      } else if (e.key === "Enter") {
        e.preventDefault();
        if (matches[pickIndex]) choose(matches[pickIndex]);
      }
      return;
    }
    if (approval && e.key === "Enter" && (e.metaKey || e.ctrlKey || approval.risk !== "danger") && !text) {
      e.preventDefault();
      api.approvalAnswer(approval.id, "allow");
      return;
    }
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      if ((e.metaKey || e.ctrlKey) && run) openFull();
      else send();
    }
  }

  function onInput() {
    picking = text.startsWith("/");
    pickIndex = 0;
  }

  function openFull() {
    if (run) api.paletteOpenRun(run.id, run.conversationId, run.agentId);
    else api.mainShow();
  }
</script>

<div class="wrap">
  <div class="card" bind:this={card}>
    <div class="bar">
      {#if agent}
        <button class="who" onclick={() => { picking = !picking; text = picking ? "/" : ""; input?.focus(); }} title="Cambia agente (/)">
          <AgentDisc icon={agent.icon} hue={agent.hue} size={26} />
          <span>{agent.name}</span>
        </button>
      {/if}
      <!-- svelte-ignore a11y_autofocus -->
      <input
        bind:this={input}
        bind:value={text}
        oninput={onInput}
        onkeydown={onKey}
        placeholder={status === "running" ? "Sta lavorando… (⌘↵ per aprirlo in Oltremare)" : `Chiedi a ${agent?.name ?? "Oltremare"}…  («/» per cambiare agente)`}
        spellcheck="false"
        autofocus
      />
      {#if status === "running"}
        <span class="spin" aria-label="In corso"></span>
      {/if}
    </div>

    {#if picking}
      <ul class="pick" role="listbox">
        {#each matches as a, i (a.id)}
          <li>
            <button role="option" aria-selected={i === pickIndex} class:on={i === pickIndex} onclick={() => choose(a)}>
              <AgentDisc icon={a.icon} hue={a.hue} size={22} />
              <strong>{a.name}</strong>
              <small>{a.description}</small>
            </button>
          </li>
        {:else}
          <li class="none">Nessun agente con «{query}»</li>
        {/each}
      </ul>
    {:else if status !== "idle"}
      <div class="out">
        {#if approval}
          <div class="appr">
            <Icon name="shield" size={15} />
            <span><strong>{approval.title}</strong><small>{approval.summary}</small></span>
            <button class="btn btn-quiet" onclick={() => approval && api.approvalAnswer(approval.id, "deny")}>Nega</button>
            <button class="btn btn-primary" onclick={() => approval && api.approvalAnswer(approval.id, "allow")}>Consenti</button>
          </div>
        {/if}
        {#if answer}
          <div class="md ans">{@html markdown(answer)}</div>
        {/if}
        {#if activity}<p class="act"><Icon name="bolt" size={12} /> {activity}</p>{/if}
        {#if error}<p class="errline">{error}</p>{/if}
        <div class="foot">
          <button class="link" onclick={openFull}>Apri in Oltremare <kbd>⌘↵</kbd></button>
          {#if status !== "running"}<button class="link" onclick={() => reset()}>Nuova domanda</button>{/if}
        </div>
      </div>
    {/if}
  </div>
</div>

<style>
  :global(html.palette),
  :global(html.palette body),
  :global(html.palette #app) {
    background: transparent !important;
  }
  .wrap {
    padding: 10px 12px 14px;
  }
  .card {
    border-radius: 16px;
    background: var(--paper);
    box-shadow: 0 0 0 1px var(--rule), 0 18px 40px -16px oklch(0.25 0.12 var(--accent-h) / 0.45);
    overflow: hidden;
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 58px;
    padding: 0 16px 0 12px;
  }
  .who {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 4px 9px 4px 4px;
    border-radius: 999px;
    background: var(--paper-2);
    font-size: var(--t-sm);
    font-weight: 600;
    flex: none;
  }
  input {
    flex: 1;
    min-width: 0;
    border: 0;
    outline: none;
    background: transparent;
    font-size: 1.12rem;
    color: var(--ink);
  }
  .spin {
    width: 16px;
    height: 16px;
    border-radius: 50%;
    border: 2px solid var(--accent-wash);
    border-top-color: var(--accent);
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .pick {
    list-style: none;
    margin: 0;
    padding: 4px 8px 10px;
    border-top: 1px solid var(--rule-soft);
    max-height: 300px;
    overflow-y: auto;
  }
  .pick button {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 8px;
    border-radius: var(--r-sm);
    text-align: left;
  }
  .pick button.on {
    background: var(--accent-mist);
  }
  .pick strong {
    font-weight: 600;
    flex: none;
  }
  .pick small {
    color: var(--ink-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .none {
    padding: 8px;
    color: var(--ink-3);
    font-size: var(--t-sm);
  }
  .out {
    border-top: 1px solid var(--rule-soft);
    padding: 12px 18px 12px;
    display: grid;
    gap: 10px;
  }
  .ans {
    max-height: 340px;
    overflow-y: auto;
  }
  .act {
    margin: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
  .errline {
    margin: 0;
    color: var(--danger);
    font-size: var(--t-sm);
  }
  .appr {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    border-radius: var(--r-md);
    background: var(--accent-mist);
    color: var(--accent);
  }
  .appr span {
    flex: 1;
    min-width: 0;
    display: grid;
    color: var(--ink);
  }
  .appr small {
    color: var(--ink-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .foot {
    display: flex;
    justify-content: space-between;
    font-size: var(--t-xs);
  }
  kbd {
    font-family: inherit;
    font-size: 0.68rem;
    color: var(--ink-3);
  }
</style>
