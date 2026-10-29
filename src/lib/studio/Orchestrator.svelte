<script lang="ts">
  // L'orchestratore: per ogni richiesta sceglie tipo, modello e piano (modificabili), segue Claude dal vivo,
  // mostra il piano da approvare e, su richiesta, il secondo parere di un'altra AI (Claudex).
  import { tick } from "svelte";
  import Icon from "../Icon.svelte";
  import { markdown } from "../markdown";
  import { dev } from "../state/dev.svelte";
  import { studio, type Task } from "../state/studio.svelte";
  import { settings } from "../state/settings.svelte";
  import ToolGlyph, { toolKind, TOOL_LABEL } from "./ToolGlyph.svelte";
  import FileIcon from "../dev/FileIcon.svelte";
  import type { DevItem } from "../api";

  let { prompt = "" }: { prompt?: string } = $props();
  let text = $state("");
  let notes = $state("");
  let opinionProfile = $state("");
  let openItems = $state<Record<number, boolean>>({});
  let scroller: HTMLElement | undefined = $state();

  const KIND: Record<string, string> = { domanda: "Domanda", piccola: "Modifica piccola", media: "Lavoro medio", grande: "Lavoro grande" };
  const MODEL: Record<string, string> = { haiku: "Haiku", sonnet: "Sonnet", opus: "Opus", fable: "Fable", auto: "Auto" };
  const items = $derived(dev.session?.items ?? []);
  /** Le azioni consecutive di Claude si raggruppano in un blocco con la guida di rientro */
  const blocks = $derived.by(() => {
    const out: { kind: "user" | "text" | "tools"; start: number; items: DevItem[] }[] = [];
    items.forEach((it, i) => {
      const k = it.kind === "tool" ? "tools" : it.kind;
      const last = out.at(-1);
      if (k === "tools" && last?.kind === "tools") last.items.push(it);
      else out.push({ kind: k, start: i, items: [it] });
    });
    return out;
  });
  /** Colore di ogni modello e tipo di richiesta (lo stesso nei selettori) */
  const MODEL_HUE: Record<string, number> = { haiku: 150, sonnet: 240, opus: 290, fable: 80, auto: 262 };
  const KIND_HUE: Record<string, number> = { domanda: 195, piccola: 150, media: 240, grande: 35 };
  /** Un'azione su un file: icona del file, nome e cartella */
  function fileOf(title: string | undefined) {
    const t = (title ?? "").trim();
    if (!t || /\s/.test(t) || !/\.[a-z0-9]{1,6}$/i.test(t)) return null;
    const parts = t.split(/[\\/]/);
    const name = parts.pop() ?? t;
    return { name, dir: parts.slice(-2).join("/") };
  }
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
    if (!t) return;
    text = "";
    // Claude sta lavorando (o c'è già una coda): il messaggio aspetta il suo turno
    if (studio.busy || studio.queue.length) {
      studio.enqueue(t);
      studio.drain();
      return;
    }
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
    <span class="hicon"><Icon name="spark" size={14} /></span>
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
    {#each blocks as b (b.start)}
      {#if b.kind === "user"}
        <div class="u">{b.items[0].text}</div>
      {:else if b.kind === "text"}
        <div class="md a">{@html markdown(b.items[0].text ?? "")}</div>
      {:else}
        <ol class="acts" aria-label="{b.items.length} azioni di Claude">
          {#each b.items as it, j (j)}
            {@const i = b.start + j}
            {@const k = toolKind(it.tool ?? "")}
            {@const f = fileOf(it.title)}
            <li>
              <button class="act" class:err={it.error} class:open={openItems[i]} onclick={() => (openItems[i] = !openItems[i])} disabled={!it.result}>
                <ToolGlyph kind={k} />
                <span class="verb">{k === "mcp" || k === "other" ? it.tool : TOOL_LABEL[k]}</span>
                {#if f}
                  <FileIcon name={f.name} size={14} />
                  <span class="fname">{f.name}</span>
                  {#if f.dir}<span class="fdir">{f.dir}</span>{/if}
                {:else}
                  <span class="target" class:cmd={k === "bash"}>{it.title}</span>
                {/if}
                <span class="state">
                  {#if !it.done}<span class="spin"></span>{:else if it.error}<span class="pill bad">errore</span>{:else}<span class="pill ok"><Icon name="check" size={10} /></span>{/if}
                </span>
              </button>
              {#if openItems[i] && it.result}<pre class="res">{it.result}</pre>{/if}
            </li>
          {/each}
        </ol>
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
            <span style="--h:{KIND_HUE[t.decision.kind] ?? 262}">{KIND[t.decision.kind] ?? t.decision.kind}</span>
            <span style="--h:{MODEL_HUE[t.model] ?? 262}"><span class="mdot"></span>{MODEL[t.model] ?? t.model}</span>
            <span style="--h:285">{t.plan ? "con piano" : "senza piano"}</span>
            <span style="--h:{t.profile ? 35 : 275}">{aiName(t.profile)}</span>
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
    {#if studio.queue.length}
      <ol class="queue" aria-label="Messaggi in coda">
        {#each studio.queue as q, i (q.id)}
          <li>
            <span class="qn">{i + 1}</span>
            <span class="qt" title={q.text}>{q.text}</span>
            <button class="qa" onclick={() => { studio.unqueue(q.id); text = q.text; }} title="Riporta nel campo per modificarlo" aria-label="Modifica"><Icon name="pen" size={12} /></button>
            <button class="qa" onclick={() => studio.sendNow(q.id)} title={running ? "Interrompi il lavoro in corso e invialo subito" : "Invialo subito"} aria-label="Invia subito"><Icon name="bolt" size={12} /></button>
            <button class="qa" onclick={() => studio.unqueue(q.id)} title="Togli dalla coda" aria-label="Togli"><Icon name="x" size={12} /></button>
          </li>
        {/each}
        <li class="qinfo">{studio.lastTask?.phase === "plan-ready" ? "In attesa: partono dopo che hai approvato il piano e Claude ha finito" : "Partono in ordine appena Claude finisce"}</li>
      </ol>
    {/if}
    <textarea class="input" rows="3" bind:value={text} onkeydown={key} placeholder={studio.busy ? "Claude sta lavorando: scrivi pure, il messaggio va in coda" : `Cosa facciamo su ${studio.project?.name ?? "questo progetto"}?`}></textarea>
    <div class="ctl">
      <label title="Modello: Auto lascia scegliere all'orchestratore" style="--h:{MODEL_HUE[studio.modelChoice]}">
        <Icon name="cpu" size={13} />
        <select bind:value={studio.modelChoice} aria-label="Modello">
          <option value="auto">Modello auto</option>
          <option value="haiku">Haiku</option>
          <option value="sonnet">Sonnet</option>
          <option value="opus">Opus</option>
          <option value="fable">Fable</option>
        </select>
      </label>
      <label title="Piano: Auto decide in base alla richiesta" style="--h:285">
        <Icon name="all" size={13} />
        <select bind:value={studio.planChoice} aria-label="Piano">
          <option value="auto">Piano auto</option>
          <option value="yes">Sempre piano</option>
          <option value="no">Senza piano</option>
        </select>
      </label>
      <label title="Quale AI fa il lavoro" style="--h:{studio.profile ? 35 : 275}">
        <Icon name="spark" size={13} />
        <select bind:value={studio.profile} aria-label="AI">
          <option value="">Claude</option>
          {#each profiles as p (p.name)}<option value={p.name}>{p.name}{p.model ? ` · ${p.model}` : ""}</option>{/each}
        </select>
      </label>
      <span class="sp"></span>
      <button class="btn btn-primary sm" onclick={() => send()} disabled={!text.trim()}>{studio.busy ? "Accoda" : "Invia"}</button>
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
  .hicon {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 7px;
    color: var(--accent);
    background: color-mix(in oklch, var(--accent) 14%, transparent);
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
    padding: 9px 12px;
    border-radius: 14px 14px 4px 14px;
    background: linear-gradient(135deg, color-mix(in oklch, var(--accent) 16%, var(--paper)), color-mix(in oklch, var(--accent) 9%, var(--paper)));
    white-space: pre-wrap;
    font-size: var(--t-sm);
  }
  .a {
    font-size: var(--t-sm);
  }
  .acts {
    position: relative;
    list-style: none;
    margin: 2px 0;
    padding: 0 0 0 10px;
    display: grid;
    gap: 1px;
  }
  /* Guida di rientro, come nell'esplora file */
  .acts::before {
    content: "";
    position: absolute;
    top: 6px;
    bottom: 6px;
    left: 3px;
    width: 1px;
    background: var(--rule-soft);
  }
  .act {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
    height: 30px;
    padding: 0 6px 0 4px;
    border-radius: 8px;
    font-size: var(--t-sm);
    text-align: left;
    color: var(--ink-2);
    transition: background-color 0.12s;
  }
  .act:not(:disabled):hover,
  .act.open {
    background: color-mix(in oklch, var(--accent) 7%, transparent);
  }
  .act:disabled {
    cursor: default;
  }
  .verb {
    flex: none;
    font-weight: 650;
    color: var(--ink);
  }
  .fname {
    flex: none;
    max-width: 45%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--ink);
  }
  .fdir,
  .target {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
  .fdir {
    direction: rtl;
    text-align: left;
  }
  .target.cmd {
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
    font-size: 11.5px;
    color: var(--ink-2);
  }
  .state {
    flex: none;
    margin-left: auto;
    display: grid;
    place-items: center;
    min-width: 18px;
  }
  .pill {
    display: inline-grid;
    place-items: center;
    height: 17px;
    min-width: 17px;
    padding: 0 5px;
    border-radius: 999px;
    font-size: 0.62rem;
    font-weight: 750;
  }
  .pill.ok {
    color: oklch(0.45 0.13 150);
    background: oklch(0.78 0.13 150 / 0.28);
  }
  .pill.bad {
    color: oklch(0.5 0.17 25);
    background: oklch(0.75 0.14 25 / 0.25);
  }
  :global([data-theme="dark"]) .pill.ok {
    color: oklch(0.85 0.13 150);
  }
  :global([data-theme="dark"]) .pill.bad {
    color: oklch(0.82 0.13 25);
  }
  .act.err .verb {
    color: oklch(0.55 0.17 25);
  }
  .res {
    margin: 2px 0 6px 33px;
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
  .chips > span {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 2px 9px;
    border-radius: 999px;
    font-size: 0.7rem;
    font-weight: 650;
    color: oklch(0.45 0.14 var(--h));
    background: oklch(0.75 0.12 var(--h) / 0.2);
  }
  :global([data-theme="dark"]) .chips > span {
    color: oklch(0.85 0.11 var(--h));
    background: oklch(0.6 0.12 var(--h) / 0.25);
  }
  .mdot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: oklch(0.6 0.16 var(--h));
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
  .queue {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 4px;
  }
  .queue li {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    padding: 5px 6px 5px 8px;
    border-radius: var(--r-sm);
    background: var(--accent-mist);
    font-size: var(--t-sm);
  }
  .queue li.qinfo {
    background: none;
    padding: 0 2px;
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
  .qn {
    flex: none;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    font-size: 0.66rem;
    font-weight: 700;
    color: var(--accent);
    background: var(--paper);
  }
  .qt {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .qa {
    flex: none;
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 6px;
    color: var(--ink-3);
  }
  .qa:hover {
    color: var(--accent);
    background: var(--paper);
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
    background: oklch(0.75 0.12 var(--h, 262) / 0.16);
    color: oklch(0.5 0.14 var(--h, 262));
    transition: background-color 0.15s, color 0.15s;
  }
  :global([data-theme="dark"]) .ctl label {
    background: oklch(0.6 0.12 var(--h, 262) / 0.22);
    color: oklch(0.82 0.11 var(--h, 262));
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
