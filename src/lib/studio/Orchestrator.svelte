<script lang="ts">
  // L'orchestratore: per ogni richiesta sceglie tipo, modello e piano (modificabili), segue Claude dal vivo,
  // mostra il piano da approvare e, su richiesta, il secondo parere di un'altra AI (Claudex).
  import { tick } from "svelte";
  import Icon from "../Icon.svelte";
  import { markdown } from "../markdown";
  import { studio, type Task } from "../state/studio.svelte";
  import { secondReviewer } from "../state/workspace.svelte";
  import { settings } from "../state/settings.svelte";
  import ToolGlyph, { toolKind, TOOL_LABEL } from "./ToolGlyph.svelte";
  import FileIcon from "../dev/FileIcon.svelte";
  import { api, type DevItem } from "../api";
  import { copia } from "../clipboard";
  import { getContext } from "svelte";
  import type { Workspace as WS } from "../state/workspace.svelte";

  const ws = getContext<WS>("ws");

  let { prompt = "" }: { prompt?: string } = $props();
  let text = $state("");
  let notes = $state("");
  let opinionProfile = $state("");
  let history = $state(false);

  function quando(ms: number) {
    const d = new Date(ms);
    const oggi = new Date();
    const stesso = d.toDateString() === oggi.toDateString();
    return stesso ? d.toLocaleTimeString("it-IT", { hour: "2-digit", minute: "2-digit" }) : d.toLocaleDateString("it-IT", { day: "numeric", month: "short" });
  }

  function outside(node: HTMLElement) {
    const h = (e: PointerEvent) => {
      if (!node.parentElement?.contains(e.target as Node)) history = false;
    };
    const k = (e: KeyboardEvent) => e.key === "Escape" && (history = false);
    setTimeout(() => window.addEventListener("pointerdown", h));
    window.addEventListener("keydown", k);
    return {
      destroy() {
        window.removeEventListener("pointerdown", h);
        window.removeEventListener("keydown", k);
      },
    };
  }
  let openItems = $state<Record<number, boolean>>({});
  let scroller: HTMLElement | undefined = $state();

  const KIND: Record<string, string> = { domanda: "Domanda", piccola: "Modifica piccola", media: "Lavoro medio", grande: "Lavoro grande" };
  const MODEL: Record<string, string> = { haiku: "Haiku", sonnet: "Sonnet", opus: "Opus", fable: "Fable", auto: "Auto" };
  const items = $derived(ws.dev.session?.items ?? []);
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
  const running = $derived(!!ws.dev.session?.running);
  const profiles = $derived(studio.claudex?.profiles ?? []);
  const aiName = (p: string) => (p ? p : "Claude");

  // I compiti si salvano per progetto: riaprendolo, la conversazione continua
  $effect(() => {
    void JSON.stringify(ws.tasks);
    void ws.fresh;
    ws.persist();
  });

  // Fine del lavoro di Claude: piano pronto o lavoro concluso
  let prima = false;
  $effect(() => {
    const r = running;
    if (prima && !r) ws.onFinished();
    prima = r;
  });

  // Si scende in fondo quando arrivano messaggi
  $effect(() => {
    void items.length;
    void ws.lastTask?.phase;
    tick().then(() => {
      if (scroller && scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < 200) scroller.scrollTop = scroller.scrollHeight;
    });
  });

  // Una richiesta passata dallo Sviluppatore, dalla finestra principale o da una issue
  $effect(() => {
    if (prompt) text = prompt;
  });
  $effect(() => {
    if (ws.draft) {
      text = ws.draft;
      ws.draft = "";
    }
  });

  async function send(nuova = false) {
    const t = text.trim();
    if (!t) return;
    text = "";
    // Claude sta lavorando (o c'è già una coda): il messaggio aspetta il suo turno
    if (ws.busy || ws.queue.length) {
      ws.enqueue(t);
      ws.drain();
      return;
    }
    await ws.send(t, nuova);
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
        return `${t.fixRounds ? "Correzioni in corso" : "Al lavoro"} con ${MODEL[t.model] ?? t.model}${t.profile ? ` (${t.profile})` : ""}…`;
      case "reviewing":
        return t.reviews?.length ? `${secondReviewer(t.profile)?.name ?? "Un'altra AI"} rivede le modifiche…` : `Il CTO (${t.model === "fable" ? "Fable" : "Opus"}) rivede le modifiche…`;
      case "review-ready":
        return "Il CTO chiede ancora correzioni: decidi tu";
      case "done":
        if (t.dropped) return "Piano scartato";
        return t.review?.esito === "approvato" ? `Fatto · approvato dal CTO${t.review.voto ? ` (${t.review.voto}/10)` : ""}` : "Fatto";
      case "error":
        return t.error ?? "Errore";
    }
  }
</script>

<section class="orch">
  <header>
    <span class="hicon"><Icon name="spark" size={14} /></span>
    <strong>Orchestratore</strong>
    {#if running}<span class="live"><span class="pulse"></span>{ws.dev.session?.stuck ? "fermo" : "al lavoro"}</span>{/if}
    <span class="sp"></span>
    {#if running && ws.dev.session?.here}
      <button class="icon-btn" title="Ferma Claude" aria-label="Ferma" onclick={() => ws.dev.stopClaude()}><Icon name="stop" size={14} /></button>
    {/if}
    {#if (ws.dev.session?.sessions ?? []).some((x) => !x.review)}
      <div class="hist">
        <button class="icon-btn" class:on={history} onclick={() => (history = !history)} title="Conversazioni passate di questo progetto" aria-label="Conversazioni passate" aria-expanded={history}><Icon name="clock" size={15} /></button>
        {#if history}
          <div class="hmenu" role="menu" use:outside>
            <div class="hh">Conversazioni di {ws.project.name}</div>
            {#each (ws.dev.session?.sessions ?? []).filter((x) => !x.review) as sx (sx.id)}
              <button role="menuitem" class:cur={sx.id === ws.dev.session?.id} onclick={() => { ws.switchSession(sx.id); history = false; }}>
                <span class="ht">{sx.title || "Senza titolo"}</span>
                <small>{quando(sx.updatedAt)}</small>
              </button>
            {/each}
          </div>
        {/if}
      </div>
    {/if}
    {#if ws.memoryNote}
      <button class="icon-btn" title="Memoria del progetto: com'è fatto, decisioni, diario dei lavori (si apre in Obsidian)" aria-label="Memoria del progetto" onclick={() => api.memoryOpen(ws.memoryNote ?? undefined)}><Icon name="book" size={15} /></button>
    {/if}
    <button class="icon-btn" title="Nuova conversazione" aria-label="Nuova conversazione" onclick={() => { ws.newConversation(); text = ""; }}><Icon name="plus" size={15} /></button>
  </header>

  <div class="msgs" bind:this={scroller}>
    {#if !items.length && !ws.tasks.length}
      <div class="intro">
        <p>Scrivi cosa vuoi fare su <strong>{ws.project?.name}</strong>. L'orchestratore decide se serve un piano e quale modello usare:</p>
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
        <div class="u">
          {b.items[0].text}
          <button class="cp" onclick={() => copia(b.items[0].text ?? "")} title="Copia il messaggio" aria-label="Copia il messaggio"><Icon name="copy" size={12} /></button>
        </div>
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

    {#if ws.lastTask}
      {@const t = ws.lastTask}
      <div class="task ph-{t.phase}">
        <div class="trow">
          {#if t.phase === "deciding" || t.phase === "planning" || t.phase === "working" || t.phase === "reviewing"}<span class="spin"></span>{:else if t.phase === "error"}<Icon name="alert" size={14} />{:else if t.phase === "review-ready"}<Icon name="shield" size={14} />{:else}<Icon name="check" size={14} />{/if}
          <span>{fase(t)}</span>
          {#if t.phase === "planning" || t.phase === "working" || t.phase === "reviewing"}
            <button class="halt" onclick={() => ws.halt(t)} title="Ferma e chiudi questo compito"><Icon name="stop" size={11} /> Ferma</button>
          {/if}
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
            <button class="btn btn-primary sm" onclick={() => { ws.approve(t, notes); notes = ""; }}><Icon name="play" size={12} /> Approva ed esegui</button>
            <select class="input sel" bind:value={t.model} aria-label="Modello per l'esecuzione">
              <option value="haiku">con Haiku</option>
              <option value="sonnet">con Sonnet</option>
              <option value="opus">con Opus</option>
              <option value="fable">con Fable</option>
            </select>
            <button class="btn btn-quiet sm" disabled={!notes.trim()} onclick={() => { ws.replan(t, notes); notes = ""; }}>Rifai il piano</button>
            <button class="btn btn-quiet sm drop" onclick={() => { ws.discard(t); notes = ""; }} title="Non eseguire questo piano">Lascia perdere</button>
          </div>
        {/if}

        {#if t.phase === "reviewing" && t.reviews?.length}
          <small class="why">Il CTO ha finito: ora rivede anche {secondReviewer(t.profile)?.name ?? "un'altra AI"}…</small>
        {/if}
        {#if t.reviews?.length && t.phase !== "reviewing" && t.phase !== "working"}
          {#each t.reviews as rv, ri (ri)}
            {@const r = rv.review}
            <div class="review" class:ko={r.esito === "da correggere"} class:unk={r.esito === "sconosciuto"}>
              <div class="rhead">
                <span class="rbadge"><Icon name={ri === 0 ? "shield" : "users"} size={12} /> {rv.who}</span>
                <strong>{r.esito === "approvato" ? "Approvato" : r.esito === "da correggere" ? "Da correggere" : "Giudizio in prosa"}</strong>
                {#if r.voto}<span class="voto" style="--v:{r.voto}">{r.voto}<small>/10</small></span>{/if}
              </div>
              {#if r.sintesi}<p class="rsum">{r.sintesi}</p>{:else if r.esito === "sconosciuto" && r.testo}<p class="rsum">{r.testo}</p>{/if}
              {#if r.problemi.length}
                <ul class="issues">
                  {#each r.problemi as p, i (i)}
                    <li class="g-{p.gravita}">
                      <em>{p.gravita}</em>
                      <div>
                        {#if p.file}<code>{p.file}{p.riga ? `:${p.riga}` : ""}</code>{/if}
                        <span>{p.problema}</span>
                        {#if p.correzione}<small>→ {p.correzione}</small>{/if}
                      </div>
                    </li>
                  {/each}
                </ul>
              {/if}
            </div>
          {/each}
          {#if t.fixRounds}<small class="why">Le correzioni sono già partite da sole {t.fixRounds === 1 ? "una volta" : `${t.fixRounds} volte`}.</small>{/if}
          {#if t.phase === "review-ready" && t.review}
            <div class="pact">
              <button class="btn btn-primary sm" onclick={() => ws.fix(t)}><Icon name="pen" size={12} /> Fai correggere ({t.review.problemi.filter((p) => p.gravita !== "minore").length})</button>
              <button class="btn btn-quiet sm" onclick={() => ws.accept(t)}>Va bene così</button>
            </div>
          {/if}
        {/if}

        {#if studio.claudex?.installed && profiles.length && (t.phase === "plan-ready" || t.phase === "done")}
          <div class="second">
            <Icon name="users" size={13} />
            <span>Secondo parere {t.phase === "plan-ready" ? "sul piano" : "sulle modifiche"} da</span>
            <select class="input sel" bind:value={opinionProfile} aria-label="AI per il secondo parere">
              <option value="" disabled>scegli un'AI</option>
              {#each profiles as p (p.name)}<option value={p.name}>{p.name}</option>{/each}
            </select>
            <button class="btn btn-line sm" disabled={!opinionProfile || t.opinion === "…"} onclick={() => ws.opinion(t, opinionProfile, t.phase === "plan-ready" ? "plan" : "diff")}>Chiedi</button>
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
    {#if ws.queue.length}
      <ol class="queue" aria-label="Messaggi in coda">
        {#each ws.queue as q, i (q.id)}
          <li>
            <span class="qn">{i + 1}</span>
            <span class="qt" title={q.text}>{q.text}</span>
            <button class="qa" onclick={() => { ws.unqueue(q.id); text = q.text; }} title="Riporta nel campo per modificarlo" aria-label="Modifica"><Icon name="pen" size={12} /></button>
            <button class="qa" onclick={() => ws.sendNow(q.id)} title={running ? "Interrompi il lavoro in corso e invialo subito" : "Invialo subito"} aria-label="Invia subito"><Icon name="bolt" size={12} /></button>
            <button class="qa" onclick={() => ws.unqueue(q.id)} title="Togli dalla coda" aria-label="Togli"><Icon name="x" size={12} /></button>
          </li>
        {/each}
        <li class="qinfo">{ws.lastTask?.phase === "plan-ready" ? "In attesa: partono dopo che hai approvato il piano e Claude ha finito" : "Partono in ordine appena Claude finisce"}</li>
      </ol>
    {/if}
    <textarea class="input" rows="3" bind:value={text} onkeydown={key} placeholder={ws.busy ? "Claude sta lavorando: scrivi pure, il messaggio va in coda" : `Cosa facciamo su ${ws.project?.name ?? "questo progetto"}?`}></textarea>
    <div class="ctl">
      <label title="Modello: Auto lascia scegliere all'orchestratore" style="--h:{MODEL_HUE[ws.modelChoice]}">
        <Icon name="cpu" size={13} />
        <select bind:value={ws.modelChoice} aria-label="Modello">
          <option value="auto">Modello auto</option>
          <option value="haiku">Haiku</option>
          <option value="sonnet">Sonnet</option>
          <option value="opus">Opus</option>
          <option value="fable">Fable</option>
        </select>
      </label>
      <label title="Piano: Auto decide in base alla richiesta" style="--h:285">
        <Icon name="all" size={13} />
        <select bind:value={ws.planChoice} aria-label="Piano">
          <option value="auto">Piano auto</option>
          <option value="yes">Sempre piano</option>
          <option value="no">Senza piano</option>
        </select>
      </label>
      <label title="Revisione dopo ogni lavoro: il CTO (Claude Opus) e, se c'è un'altra AI in Claudex, anche lei. Auto rivede tutto tranne le domande" style="--h:{ws.reviewChoice === 'no' ? 0 : 150}">
        <Icon name="shield" size={13} />
        <select bind:value={ws.reviewChoice} aria-label="Revisione del CTO">
          <option value="auto">CTO auto</option>
          <option value="yes">CTO sempre</option>
          <option value="no">Senza CTO</option>
        </select>
      </label>
      <label title="Quale AI fa il lavoro" style="--h:{ws.profile ? 35 : 275}">
        <Icon name="spark" size={13} />
        <select bind:value={ws.profile} aria-label="AI">
          <option value="">Claude</option>
          {#each profiles as p (p.name)}<option value={p.name}>{p.name}{p.model ? ` · ${p.model}` : ""}</option>{/each}
        </select>
      </label>
      <span class="sp"></span>
      <button class="btn btn-primary sm" onclick={() => send()} disabled={!text.trim()}>{ws.busy ? "Accoda" : "Invia"}</button>
    </div>
    {#if studio.claudex && !studio.claudex.installed}
      <small class="hint">Altre AI (Codex, Gemini, Grok, DeepSeek, locali): installa <a href="https://github.com/StringKe/claudex" target="_blank" rel="noreferrer">Claudex</a> e configura i profili; compaiono qui.</small>
    {:else if studio.claudex?.installed && !profiles.length}
      <small class="hint">Claudex è installato ma senza profili: aggiungili con <code>claudex profile add</code> (o in ~/.config/claudex/config.toml) e torna qui.</small>
    {:else if profiles.length && !ws.profile}
      <small class="hint">Altre AI via Claudex nel menu qui sopra: {profiles.map((p) => p.name).join(", ")}.</small>
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
    position: relative;
    align-self: flex-end;
    max-width: 90%;
    padding: 9px 12px;
    border-radius: 14px 14px 4px 14px;
    background: linear-gradient(135deg, color-mix(in oklch, var(--accent) 16%, var(--paper)), color-mix(in oklch, var(--accent) 9%, var(--paper)));
    white-space: pre-wrap;
    font-size: var(--t-sm);
    user-select: text;
    -webkit-user-select: text;
    overflow-wrap: anywhere;
  }
  .cp {
    position: absolute;
    top: -8px;
    left: -8px;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    color: var(--ink-3);
    background: var(--paper);
    box-shadow: 0 1px 3px oklch(0.3 0.05 var(--tint-h) / 0.18), inset 0 0 0 1px var(--rule-soft);
    opacity: 0;
    transition: opacity 0.12s, color 0.12s;
  }
  .u:hover .cp,
  .cp:focus-visible {
    opacity: 1;
  }
  .cp:hover {
    color: var(--accent);
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
  .halt {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    margin-left: auto;
    padding: 2px 8px;
    border-radius: 999px;
    font-size: var(--t-xs);
    font-weight: 600;
    color: var(--ink-3);
    box-shadow: inset 0 0 0 1px var(--rule);
  }
  .halt:hover {
    color: var(--danger);
    background: var(--danger-wash);
    box-shadow: inset 0 0 0 1px color-mix(in oklch, var(--danger) 40%, transparent);
  }
  .drop {
    margin-left: auto;
    color: var(--ink-3);
  }
  .drop:hover {
    color: var(--danger);
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
  .hist {
    position: relative;
  }
  .hmenu {
    position: absolute;
    z-index: 20;
    top: calc(100% + 6px);
    right: 0;
    width: 320px;
    max-height: 60vh;
    overflow: auto;
    padding: 6px;
    border-radius: 12px;
    background: var(--paper);
    box-shadow:
      0 0 0 1px var(--rule),
      0 12px 32px -8px oklch(0.25 0.06 var(--tint-h) / 0.28);
  }
  .hh {
    padding: 4px 8px 6px;
    font-size: var(--t-xs);
    font-weight: 700;
    color: var(--ink-2);
  }
  .hmenu button[role="menuitem"] {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 8px;
    border-radius: 7px;
    font-size: var(--t-sm);
    text-align: left;
  }
  .hmenu button[role="menuitem"]:hover {
    background: color-mix(in oklch, var(--accent) 9%, transparent);
  }
  .hmenu button.cur {
    box-shadow: inset 2px 0 0 var(--accent);
    background: color-mix(in oklch, var(--accent) 6%, transparent);
  }
  .ht {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .hmenu small {
    flex: none;
    color: var(--ink-3);
  }
  .task.ph-review-ready {
    box-shadow: inset 0 0 0 1.5px var(--warn);
  }
  .review {
    display: grid;
    gap: 8px;
    padding: 10px 12px;
    border-radius: var(--r-sm);
    background: var(--paper);
    font-size: var(--t-sm);
    box-shadow: inset 0 0 0 1px color-mix(in oklch, var(--ok) 35%, transparent);
  }
  .review.ko {
    box-shadow: inset 0 0 0 1px color-mix(in oklch, var(--warn) 55%, transparent);
  }
  .review.unk {
    box-shadow: inset 0 0 0 1px var(--rule);
  }
  .rhead {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .rbadge {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 8px;
    border-radius: 999px;
    font-size: 0.68rem;
    font-weight: 750;
    letter-spacing: 0.04em;
    color: var(--accent);
    background: color-mix(in oklch, var(--accent) 12%, transparent);
  }
  .voto {
    margin-left: auto;
    font-weight: 750;
    font-variant-numeric: tabular-nums;
    color: oklch(0.5 0.14 calc(20 + var(--v) * 13));
  }
  .voto small {
    font-weight: 500;
    color: var(--ink-3);
  }
  .rounds {
    color: var(--ink-3);
    font-size: var(--t-xs);
  }
  .rsum {
    margin: 0;
    color: var(--ink-2);
    line-height: 1.45;
  }
  .issues {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 6px;
  }
  .issues li {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    padding: 6px 8px;
    border-radius: 7px;
    background: var(--paper-2);
  }
  .issues em {
    flex: none;
    min-width: 72px;
    padding: 1px 7px;
    border-radius: 999px;
    text-align: center;
    font-style: normal;
    font-size: 0.64rem;
    font-weight: 750;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .g-bloccante em {
    color: var(--danger);
    background: var(--danger-wash);
  }
  .g-importante em {
    color: var(--warn-ink);
    background: oklch(0.8 0.12 80 / 0.3);
  }
  .g-minore em {
    color: var(--ink-3);
    background: var(--paper-3);
  }
  .issues div {
    display: grid;
    gap: 2px;
    min-width: 0;
  }
  .issues code {
    font-size: 0.72rem;
    color: var(--accent);
  }
  .issues small {
    color: var(--ink-3);
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
