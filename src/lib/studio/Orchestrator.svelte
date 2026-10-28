<script lang="ts">
  // L'orchestratore: per ogni richiesta sceglie tipo, modello e piano (modificabili), segue Claude dal vivo,
  // mostra il piano da approvare e, su richiesta, il secondo parere di un'altra AI (Claudex).
  import { tick } from "svelte";
  import Icon from "../Icon.svelte";
  import { markdown } from "../markdown";
  import { dev } from "../state/dev.svelte";
  import { studio, type Task } from "../state/studio.svelte";
  import { settings } from "../state/settings.svelte";

  let { prompt = "" }: { prompt?: string } = $props();
  let text = $state("");
  let notes = $state("");
  let opinionProfile = $state("");
  let openItems = $state<Record<number, boolean>>({});
  let scroller: HTMLElement | undefined = $state();

  const KIND: Record<string, string> = { domanda: "Domanda", piccola: "Modifica piccola", media: "Lavoro medio", grande: "Lavoro grande" };
  const MODEL: Record<string, string> = { haiku: "Haiku", sonnet: "Sonnet", opus: "Opus", fable: "Fable", auto: "Auto" };
  const items = $derived(dev.session?.items ?? []);
  const running = $derived(!!dev.session?.running);
  const profiles = $derived(studio.claudex?.profiles ?? []);
  const aiName = (p: string) => (p ? p : "Claude");

  // Fine del lavoro di Claude: piano pronto o lavoro concluso
  let prima = false;
  $effect(() => {
    const r = running;
    if (prima && !r) studio.onFinished();
    prima = r;
  });

  // Si scende in fondo quando arrivano messaggi
  $effect(() => {
    void items.length;
    void studio.lastTask?.phase;
    tick().then(() => {
      if (scroller && scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < 200) scroller.scrollTop = scroller.scrollHeight;
    });
  });

  // Una richiesta passata dallo Sviluppatore, dalla finestra principale o da una issue
  $effect(() => {
    if (prompt) text = prompt;
  });
  $effect(() => {
    if (studio.draft) {
      text = studio.draft;
      studio.draft = "";
    }
  });

  async function send(nuova = false) {
    const t = text.trim();
    if (!t || running) return;
    text = "";
    await studio.send(t, nuova);
  }

  function key(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey && (settings.s?.sendWithEnter ?? true)) {
      e.preventDefault();
      send();
    }
  }

  function fase(t: Task) {
    switch (t.phase) {
      case "deciding":
        return "L'orchestratore valuta la richiesta…";
      case "planning":
        return `Preparo il piano con ${MODEL[t.model] ?? t.model}${t.profile ? ` (${t.profile})` : ""}…`;
      case "plan-ready":
        return "Piano pronto: approvalo o chiedi modifiche";
      case "working":
        return `Al lavoro con ${MODEL[t.model] ?? t.model}${t.profile ? ` (${t.profile})` : ""}…`;
      case "done":
        return "Fatto";
      case "error":
        return t.error ?? "Errore";
    }
  }
</script>

<section class="orch">
  <header>
    <strong>Orchestratore</strong>
    {#if running}<span class="live"><span class="pulse"></span>{dev.session?.stuck ? "fermo" : "al lavoro"}</span>{/if}
    <span class="sp"></span>
    {#if running && dev.session?.here}
      <button class="icon-btn" title="Ferma Claude" aria-label="Ferma" onclick={() => dev.stopClaude()}><Icon name="stop" size={14} /></button>
    {/if}
    <button class="icon-btn" title="Nuova conversazione" aria-label="Nuova conversazione" onclick={() => { dev.sessionId = null; studio.tasks = []; text = ""; }}><Icon name="plus" size={15} /></button>
  </header>

  <div class="msgs" bind:this={scroller}>
    {#if !items.length && !studio.tasks.length}
      <div class="intro">
        <p>Scrivi cosa vuoi fare su <strong>{studio.project?.name}</strong>. L'orchestratore decide se serve un piano e quale modello usare:</p>
        <ul>
          <li><b>Haiku</b> per le modifiche piccole e ovvie</li>
          <li><b>Sonnet</b> per domande e lavori circoscritti</li>
          <li><b>Opus</b> con piano per i lavori grandi</li>
          <li><b>Fable</b>, il più capace, per i problemi più difficili</li>
        </ul>
        <p>Con il piano, Claude prima analizza senza toccare nulla; quando approvi, scrive il codice.</p>
      </div>
    {/if}
    {#each items as it, i (i)}
      {#if it.kind === "user"}
        <div class="u">{it.text}</div>
      {:else if it.kind === "text"}
        <div class="md a">{@html markdown(it.text ?? "")}</div>
      {:else}
        <button class="tool" class:err={it.error} onclick={() => (openItems[i] = !openItems[i])}>
          {#if !it.done}<span class="spin"></span>{:else if it.error}<Icon name="x" size={12} />{:else}<Icon name="check" size={12} />{/if}
          <strong>{it.tool}</strong><span>{it.title}</span>
        </button>
        {#if openItems[i] && it.result}<pre class="res">{it.result}</pre>{/if}
      {/if}
    {/each}

    {#if studio.lastTask}
      {@const t = studio.lastTask}
      <div class="task ph-{t.phase}">
        <div class="trow">
          {#if t.phase === "deciding" || t.phase === "planning" || t.phase === "working"}<span class="spin"></span>{:else if t.phase === "error"}<Icon name="alert" size={14} />{:else}<Icon name="check" size={14} />{/if}
          <span>{fase(t)}</span>
        </div>
        {#if t.decision}
          <div class="chips">
            <span class="k-{t.decision.kind}">{KIND[t.decision.kind] ?? t.decision.kind}</span>
            <span>{MODEL[t.model] ?? t.model}</span>
            <span>{t.plan ? "con piano" : "senza piano"}</span>
            <span>{aiName(t.profile)}</span>
          </div>
          {#if t.decision.reason}<small class="why">{t.decision.reason}</small>{/if}
        {/if}

        {#if t.phase === "plan-ready"}
          <div class="plan">
            <div class="md">{@html markdown(t.planText || "Il piano è nella conversazione qui sopra.")}</div>
          </div>
          <textarea class="input" rows="2" bind:value={notes} placeholder="Indicazioni sul piano (facoltative)"></textarea>
          <div class="pact">
            <button class="btn btn-primary sm" onclick={() => { studio.approve(t, notes); notes = ""; }}><Icon name="play" size={12} /> Approva ed esegui</button>
            <select class="input sel" bind:value={t.model} aria-label="Modello per l'esecuzione">
              <option value="haiku">con Haiku</option>
              <option value="sonnet">con Sonnet</option>
              <option value="opus">con Opus</option>
              <option value="fable">con Fable</option>
            </select>
            <button class="btn btn-quiet sm" disabled={!notes.trim()} onclick={() => { studio.replan(t, notes); notes = ""; }}>Rifai il piano</button>
          </div>
        {/if}

        {#if studio.claudex?.installed && profiles.length && (t.phase === "plan-ready" || t.phase === "done")}
          <div class="second">
            <Icon name="users" size={13} />
            <span>Secondo parere {t.phase === "plan-ready" ? "sul piano" : "sulle modifiche"} da</span>
            <select class="input sel" bind:value={opinionProfile} aria-label="AI per il secondo parere">
              <option value="" disabled>scegli un'AI</option>
              {#each profiles as p (p.name)}<option value={p.name}>{p.name}</option>{/each}
            </select>
            <button class="btn btn-line sm" disabled={!opinionProfile || t.opinion === "…"} onclick={() => studio.opinion(t, opinionProfile, t.phase === "plan-ready" ? "plan" : "diff")}>Chiedi</button>
          </div>
        {/if}
        {#if t.opinion}
          <div class="opinion">
            {#if t.opinion === "…"}<span class="spin"></span> {opinionProfile} sta leggendo…{:else}<strong>Parere di {opinionProfile}</strong><div class="md">{@html markdown(t.opinion)}</div>{/if}
          </div>
        {/if}
      </div>
    {/if}
  </div>

  <div class="composer">
    <textarea class="input" rows="3" bind:value={text} onkeydown={key} placeholder={running ? "Claude sta lavorando…" : `Cosa facciamo su ${studio.project?.name ?? "questo progetto"}?`}></textarea>
    <div class="ctl">
      <label title="Modello: Auto lascia scegliere all'orchestratore">
        <Icon name="cpu" size={13} />
        <select bind:value={studio.modelChoice} aria-label="Modello">
          <option value="auto">Modello auto</option>
          <option value="haiku">Haiku</option>
          <option value="sonnet">Sonnet</option>
          <option value="opus">Opus</option>
          <option value="fable">Fable</option>
        </select>
      </label>
      <label title="Piano: Auto decide in base alla richiesta">
        <Icon name="all" size={13} />
        <select bind:value={studio.planChoice} aria-label="Piano">
          <option value="auto">Piano auto</option>
          <option value="yes">Sempre piano</option>
          <option value="no">Senza piano</option>
        </select>
      </label>
      <label title="Quale AI fa il lavoro">
        <Icon name="spark" size={13} />
        <select bind:value={studio.profile} aria-label="AI">
          <option value="">Claude</option>
          {#each profiles as p (p.name)}<option value={p.name}>{p.name}{p.model ? ` · ${p.model}` : ""}</option>{/each}
        </select>
      </label>
      <span class="sp"></span>
      <button class="btn btn-primary sm" onclick={() => send()} disabled={!text.trim() || running}>Invia</button>
    </div>
    {#if studio.claudex && !studio.claudex.installed}
      <small class="hint">Altre AI (Codex, Gemini, Grok, DeepSeek, locali): installa <a href="https://github.com/StringKe/claudex" target="_blank" rel="noreferrer">Claudex</a> e configura i profili; compaiono qui.</small>
    {/if}
  </div>
</section>

<style>
  .orch {
    min-height: 0;
    display: flex;
    flex-direction: column;
    border-left: 1px solid var(--rule-soft);
    background: var(--paper);
  }
  header {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 40px;
    padding: 0 10px 0 14px;
    border-bottom: 1px solid var(--rule-soft);
  }
  header strong {
    font-size: var(--t-sm);
  }
  .sp {
    flex: 1;
  }
  .live {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: var(--t-xs);
    color: var(--accent);
  }
  .pulse {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--accent);
    animation: p 1s infinite alternate;
  }
  @keyframes p {
    to {
      opacity: 0.25;
    }
  }
  .msgs {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 14px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .intro {
    font-size: var(--t-sm);
    color: var(--ink-2);
    line-height: 1.5;
  }
  .intro ul {
    margin: 6px 0;
    padding-left: 18px;
  }
  .u {
    align-self: flex-end;
    max-width: 90%;
    padding: 8px 11px;
    border-radius: 12px 12px 4px 12px;
    background: var(--accent-mist);
    white-space: pre-wrap;
    font-size: var(--t-sm);
  }
  .a {
    font-size: var(--t-sm);
  }
  .tool {
    display: flex;
    align-items: center;
    gap: 7px;
    width: 100%;
    padding: 3px 6px;
    border-radius: 6px;
    font-size: var(--t-xs);
    color: var(--ink-3);
    text-align: left;
    min-width: 0;
  }
  .tool:hover {
    background: var(--paper-2);
  }
  .tool strong {
    color: var(--ink-2);
    flex: none;
  }
  .tool span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tool.err {
    color: var(--danger);
  }
  .res {
    margin: 0 0 4px 24px;
    max-height: 200px;
    overflow: auto;
    padding: 8px;
    border-radius: 6px;
    background: var(--paper-2);
    font-size: 11px;
    white-space: pre-wrap;
  }
  .spin {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    border: 2px solid var(--accent-wash);
    border-top-color: var(--accent);
    animation: s 0.8s linear infinite;
    flex: none;
    display: inline-block;
  }
  @keyframes s {
    to {
      transform: rotate(360deg);
    }
  }
  .task {
    display: grid;
    gap: 8px;
    padding: 12px;
    border-radius: var(--r-md);
    background: var(--paper-2);
    box-shadow: inset 0 0 0 1px var(--rule-soft);
  }
  .task.ph-plan-ready {
    box-shadow: inset 0 0 0 1.5px var(--accent);
  }
  .task.ph-error {
    color: var(--danger);
  }
  .trow {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--t-sm);
    font-weight: 600;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }
  .chips span {
    padding: 2px 8px;
    border-radius: 999px;
    font-size: 0.7rem;
    font-weight: 650;
    color: var(--accent);
    background: var(--accent-mist);
  }
  .chips .k-grande {
    color: oklch(0.5 0.16 30);
    background: oklch(0.65 0.15 30 / 0.14);
  }
  .chips .k-piccola {
    color: oklch(0.45 0.12 155);
    background: oklch(0.65 0.12 155 / 0.14);
  }
  .why {
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
  .plan {
    max-height: 340px;
    overflow-y: auto;
    padding: 10px 12px;
    border-radius: var(--r-sm);
    background: var(--paper);
    font-size: var(--t-sm);
  }
  .pact,
  .second {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px;
    font-size: var(--t-xs);
    color: var(--ink-2);
  }
  .sel {
    height: 28px;
    width: auto;
    font-size: var(--t-xs);
    padding: 0 6px;
  }
  .sm {
    height: 28px;
    padding: 0 10px;
    font-size: var(--t-xs);
    gap: 5px;
  }
  .opinion {
    display: grid;
    gap: 6px;
    padding: 10px 12px;
    border-radius: var(--r-sm);
    background: var(--paper);
    font-size: var(--t-sm);
  }
  .composer {
    display: grid;
    gap: 8px;
    padding: 10px 12px 12px;
    border-top: 1px solid var(--rule-soft);
  }
  .ctl {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }
  .ctl label {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 0 4px 0 8px;
    height: 28px;
    border-radius: 999px;
    background: var(--paper-2);
    color: var(--ink-3);
  }
  .ctl select {
    border: 0;
    background: transparent;
    font-size: var(--t-xs);
    color: var(--ink);
    outline: none;
  }
  .hint {
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
</style>
