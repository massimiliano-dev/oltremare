<script lang="ts">
  // Lo spazio di lavoro dello Studio: esplora file, editor a schede, differenze, anteprima, processi, orchestratore.
  import Icon from "../Icon.svelte";
  import FileTree from "../dev/FileTree.svelte";
  import CodeEditor from "./CodeEditor.svelte";
  import Orchestrator from "./Orchestrator.svelte";
  import IssuesPanel from "./IssuesPanel.svelte";
  import SourceControl from "./SourceControl.svelte";
  import DiffViewer from "./DiffViewer.svelte";
  import Terminal from "./Terminal.svelte";
  import Team from "./Team.svelte";
  import MemoryGraph from "../memory/MemoryGraph.svelte";
  import QuickOpen from "./QuickOpen.svelte";
  import SearchPanel from "./SearchPanel.svelte";
  import { setContext } from "svelte";
  import { studio } from "../state/studio.svelte";
  import type { Workspace as WS } from "../state/workspace.svelte";

  let { ws }: { ws: WS } = $props();
  // Lo spazio di lavoro di questa scheda non cambia mai (StudioApp ne monta uno per scheda)
  // svelte-ignore state_referenced_locally
  setContext("ws", ws);
  let frame: HTMLIFrameElement | undefined = $state();
  let bottomOpen = $state(true);

  // Colonne laterali allargabili a mano (trascinando il bordo, o con le frecce): la larghezza si ricorda
  const DEF = { left: 260, right: 440 };
  const LIM = { left: [180, 560], right: [320, 820] } as const;
  const leggi = (k: "left" | "right") => {
    try {
      const v = Number(localStorage.getItem(`oltremare:studio-${k}`));
      return v ? Math.min(Math.max(v, LIM[k][0]), LIM[k][1]) : DEF[k];
    } catch {
      return DEF[k];
    }
  };
  let left = $state(leggi("left"));
  let right = $state(leggi("right"));
  let grid: HTMLElement | undefined = $state();
  let dragging = $state<"left" | "right" | null>(null);

  function imposta(k: "left" | "right", v: number) {
    // Il centro tiene almeno 360 px
    const spazio = (grid?.clientWidth ?? 1400) - (k === "left" ? right : left) - 360;
    const n = Math.round(Math.min(Math.max(v, LIM[k][0]), LIM[k][1], Math.max(spazio, LIM[k][0])));
    if (k === "left") left = n;
    else right = n;
    try {
      localStorage.setItem(`oltremare:studio-${k}`, String(n));
    } catch {}
  }

  function trascina(k: "left" | "right", e: PointerEvent) {
    if (e.button !== 0 || !grid) return;
    e.preventDefault();
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
    dragging = k;
    const box = grid.getBoundingClientRect();
    const muovi = (ev: PointerEvent) => imposta(k, k === "left" ? ev.clientX - box.left : box.right - ev.clientX);
    const fine = () => {
      dragging = null;
      el.removeEventListener("pointermove", muovi);
      el.removeEventListener("pointerup", fine);
      el.removeEventListener("pointercancel", fine);
    };
    el.addEventListener("pointermove", muovi);
    el.addEventListener("pointerup", fine);
    el.addEventListener("pointercancel", fine);
  }

  function tasti(k: "left" | "right", e: KeyboardEvent) {
    const passo = e.shiftKey ? 60 : 20;
    const verso = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : 0;
    if (!verso) return;
    e.preventDefault();
    imposta(k, (k === "left" ? left : right) + (k === "left" ? verso : -verso) * passo);
  }
  // Menu dell'albero (tasto destro) e richiesta del nome per nuovo/rinomina
  let menu = $state<{ x: number; y: number; entry: { name: string; path: string; dir: boolean } } | null>(null);
  let naming = $state<{ kind: "file" | "dir" | "rename"; dir: string; from?: string; value: string } | null>(null);
  let nameInput: HTMLInputElement | undefined = $state();
  function apriMenu(e: MouseEvent, entry: { name: string; path: string; dir: boolean }) {
    menu = { x: e.clientX, y: e.clientY, entry };
  }
  function chiedi(kind: "file" | "dir" | "rename", entry: { name: string; path: string; dir: boolean } | null) {
    menu = null;
    const dir = entry ? (entry.dir ? entry.path : entry.path.slice(0, entry.path.lastIndexOf("/"))) : ws.path;
    naming = kind === "rename" && entry ? { kind, dir, from: entry.path, value: entry.name } : { kind, dir, value: "" };
    requestAnimationFrame(() => nameInput?.select());
  }
  function conferma() {
    const n = naming;
    naming = null;
    if (!n || !n.value.trim() || n.value.includes("..")) return;
    if (n.kind === "rename" && n.from) ws.renameEntry(n.from, n.value);
    else ws.createEntry(n.dir, n.value, n.kind === "dir");
  }
  const rel = (p: string) => (p.startsWith(ws.path) ? p.slice(ws.path.length + 1) : p);

  // Terminali del progetto (la shell dell'utente) accanto ai processi avviati dall'orchestratore
  let terms = $state<{ id: number; name: string; ended: boolean }[]>([]);
  let termN = 0;
  const termSel = $derived(ws.dev.bottom?.startsWith("term:") ? Number(ws.dev.bottom.slice(5)) : null);
  const proc = $derived(termSel != null ? null : (ws.dev.processes.find((p) => p.id === ws.dev.bottom) ?? ws.dev.processes[0] ?? null));

  function nuovoTerminale() {
    const id = ++termN;
    terms = [...terms, { id, name: terms.length ? `Terminale ${id}` : "Terminale", ended: false }];
    ws.dev.bottom = `term:${id}`;
    bottomOpen = true;
  }

  function chiudiTerminale(id: number) {
    terms = terms.filter((t) => t.id !== id);
    if (termSel === id) ws.dev.bottom = terms.at(-1) ? `term:${terms.at(-1)!.id}` : null;
  }

  // ⌃` mostra o nasconde il terminale, come negli editor; ⌘⇧F cerca nel progetto
  function tasto(e: KeyboardEvent) {
    if (studio.tab !== ws) return;
    if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key.toLowerCase() === "f") {
      e.preventDefault();
      ws.sidebar = "search";
      return;
    }
    if (e.key === "Escape" && (menu || naming)) {
      menu = null;
      naming = null;
      return;
    }
    if (e.ctrlKey && e.key === "`") {
      e.preventDefault();
      if (!terms.length) nuovoTerminale();
      else if (termSel != null && bottomOpen) bottomOpen = false;
      else {
        ws.dev.bottom = `term:${(terms.find((t) => t.id === termSel) ?? terms.at(-1))!.id}`;
        bottomOpen = true;
      }
    }
  }
</script>

<svelte:window onkeydown={tasto} />

<div class="ws" bind:this={grid} class:dragging style="--left:{left}px; --right:{right}px">
  <!-- Separatore che si sposta (ruolo ARIA «separator» con valore): è un controllo, si può mettere a fuoco -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
  <div class="gutter gl" class:on={dragging === "left"} role="separator" aria-orientation="vertical" aria-label="Larghezza dell'esplora file" aria-valuenow={left} aria-valuemin={LIM.left[0]} aria-valuemax={LIM.left[1]} tabindex="0" title="Trascina per allargare · doppio clic per tornare com'era" onpointerdown={(e) => trascina("left", e)} ondblclick={() => imposta("left", DEF.left)} onkeydown={(e) => tasti("left", e)}></div>
  <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
  <div class="gutter gr" class:on={dragging === "right"} role="separator" aria-orientation="vertical" aria-label="Larghezza della chat" aria-valuenow={right} aria-valuemin={LIM.right[0]} aria-valuemax={LIM.right[1]} tabindex="0" title="Trascina per allargare · doppio clic per tornare com'era" onpointerdown={(e) => trascina("right", e)} ondblclick={() => imposta("right", DEF.right)} onkeydown={(e) => tasti("right", e)}></div>
  <aside class="explorer">
    <IssuesPanel />
    <SourceControl />
    <div class="sec side">
      <button class:on={ws.sidebar === "files"} onclick={() => (ws.sidebar = "files")} title="File del progetto"><Icon name="folder" size={12} /> {ws.project?.name ?? "Esplora"}</button>
      <button class:on={ws.sidebar === "search"} onclick={() => (ws.sidebar = "search")} title="Cerca nel progetto (⌘⇧F)" aria-label="Cerca nel progetto"><Icon name="search" size={12} /></button>
      <button class:on={ws.sidebar === "problems"} onclick={() => { ws.sidebar = "problems"; if (!ws.checkedAt) ws.runCheck(); }} title="Problemi: errori del compilatore" aria-label="Problemi">
        <Icon name="alert" size={12} />{#if ws.problems.length}<span class="pcount" class:err={ws.problems.some((p) => p.severity === "error")}>{ws.problems.length}</span>{/if}
      </button>
      {#if ws.sidebar === "files"}
        <span class="sp"></span>
        <button onclick={() => chiedi("file", null)} title="Nuovo file" aria-label="Nuovo file"><Icon name="plus" size={12} /></button>
        <button onclick={() => chiedi("dir", null)} title="Nuova cartella" aria-label="Nuova cartella"><Icon name="folder" size={12} /><Icon name="plus" size={9} /></button>
      {/if}
    </div>
    {#if naming}
      <form class="naming" onsubmit={(e) => (e.preventDefault(), conferma())}>
        <small>{naming.kind === "rename" ? "Nuovo nome" : naming.kind === "dir" ? "Nuova cartella" : "Nuovo file"} in {rel(naming.dir) || ws.project.name}</small>
        <input bind:this={nameInput} bind:value={naming.value} placeholder={naming.kind === "dir" ? "nome-cartella" : "nome.ts"} onblur={() => setTimeout(() => (naming = null), 150)} aria-label="Nome" />
      </form>
    {/if}
    {#if ws.sidebar === "search"}
      <SearchPanel />
    {:else if ws.sidebar === "problems"}
      <div class="probs">
        <div class="phead">
          <small>{ws.checking ? "Controllo in corso…" : ws.checkedAt ? `${ws.checkTools.join(", ") || "nessuno strumento trovato"} · ${new Date(ws.checkedAt).toLocaleTimeString("it-IT", { hour: "2-digit", minute: "2-digit" })}` : "Controlla il progetto con i suoi strumenti"}</small>
          <button class="btn btn-quiet sm" onclick={() => ws.runCheck()} disabled={ws.checking}><Icon name="refresh" size={12} /> Controlla</button>
        </div>
        {#if ws.checkedAt && !ws.checking && !ws.problems.length}
          <p class="pok"><Icon name="check" size={12} /> {ws.checkTools.length ? "Nessun problema" : "Nessun controllo disponibile (tsc, svelte-check o cargo con le dipendenze installate)"}</p>
        {/if}
        {#each ws.problems as p, i (i)}
          <button class="prow" onclick={() => ws.openFile(p.path, p.line, p.col)} title={p.message}>
            <span class="sev sev-{p.severity}"></span>
            <span class="pm">{p.message}</span>
            <span class="pf">{rel(p.path).split("/").pop()}:{p.line}</span>
          </button>
        {/each}
      </div>
    {:else if ws.dev.folder}<FileTree path={ws.dev.folder} dev={ws.dev} onopen={(p) => ws.openFile(p)} active={ws.dev.tab === "file" ? ws.active : null} onmenu={apriMenu} />{/if}
  </aside>

  <section class="center">
    <div class="tabs">
      {#each ws.files as f (f.path)}
        <div class="tab" class:on={ws.dev.tab === "file" && ws.active === f.path}>
          <button class="tname" onclick={() => { ws.active = f.path; ws.dev.tab = "file"; }} title={f.path}>{f.name}{#if f.content !== f.saved}<span class="dirty">●</span>{/if}</button>
          <button class="tx" onclick={() => ws.closeFile(f.path)} aria-label="Chiudi {f.name}"><Icon name="x" size={11} /></button>
        </div>
      {/each}
      <div class="tab" class:on={ws.dev.tab === "diff"}><button class="tname" onclick={() => ws.git.showDiff(ws.git.diffFile, ws.git.diffStaged)}><Icon name="layers" size={12} /> Differenze{#if ws.git.changed.length + ws.git.staged.length}<span class="tcount">{ws.git.changed.length + ws.git.staged.length}</span>{/if}</button></div>
      <div class="tab" class:on={ws.dev.tab === "preview"}><button class="tname" onclick={() => (ws.dev.tab = "preview")}><Icon name="globe" size={12} /> Anteprima{ws.dev.urls.length ? ` · ${ws.dev.urls.length}` : ""}</button></div>
      <div class="tab" class:on={ws.dev.tab === "memory"}><button class="tname" onclick={() => (ws.dev.tab = "memory")}><Icon name="book" size={12} /> Memoria</button></div>
      <div class="tab" class:on={ws.dev.tab === "team"}><button class="tname" onclick={() => (ws.dev.tab = "team")}><Icon name="users" size={12} /> Squadra{#if ws.activeAgents}<span class="tcount live">{ws.activeAgents}</span>{/if}</button></div>
      <span class="tsp"></span>
      <button class="tterm" onclick={() => (terms.length ? (ws.dev.bottom = `term:${(terms.find((t) => t.id === termSel) ?? terms.at(-1))!.id}`, (bottomOpen = true)) : nuovoTerminale())} title="Terminale nella cartella del progetto (⌃`)"><Icon name="terminal" size={13} /> Terminale</button>
    </div>
    <div class="pane">
      {#if ws.dev.tab === "file"}
        {#if ws.current}
          {#if ws.current.binary}
            <p class="hint">File binario: non si può modificare qui.</p>
          {:else}
            {#key ws.current.path}
              <CodeEditor path={ws.current.path} value={ws.current.content} onchange={(v) => ws.current && (ws.current.content = v)} onsave={() => ws.save()} goto={ws.goto?.path === ws.current.path ? ws.goto : null} oncursor={(l, c, n) => (ws.cursor = { line: l, col: c, cursors: n })} diagnostics={ws.problems.filter((p) => p.path === ws.current?.path)} ondefinition={(n) => n && ws.goToDefinition(n)} />
            {/key}
            <div class="status">
              <span class="sp-path" title={ws.current.path}>{rel(ws.current.path)}</span>
              {#if ws.current.content !== ws.current.saved}<span class="dirty-s">non salvato · ⌘S</span>{/if}
              {#if ws.problems.some((p) => p.path === ws.current?.path)}{@const np = ws.problems.filter((p) => p.path === ws.current?.path).length}<button class="sb-prob" onclick={() => (ws.sidebar = "problems")}>{np} {np === 1 ? "problema" : "problemi"}</button>{/if}
              <span title="F12 o ⌘⌥+clic: vai alla definizione · ⌘D: seleziona la prossima occorrenza · ⌘+clic: aggiungi un cursore">Riga {ws.cursor.line}, col {ws.cursor.col}{ws.cursor.cursors > 1 ? ` · ${ws.cursor.cursors} cursori` : ""}</span>
              <span>{(ws.current.name.split(".").pop() ?? "").toUpperCase()}</span>
            </div>
          {/if}
        {:else}
          <div class="welcome">
            <Icon name="draft" size={26} />
            <p>Apri un file dall'esplora risorse, oppure chiedi all'orchestratore cosa fare. I file modificati da Claude compaiono in «Modifiche», pronti per il commit.</p>
          </div>
        {/if}
      {:else if ws.dev.tab === "diff"}
        <DiffViewer />
      {:else if ws.dev.tab === "team"}
        <Team />
      {:else if ws.dev.tab === "memory"}
        <MemoryGraph compact scoped focus={ws.memoryNote} />
      {:else}
        {#if ws.dev.urls.length}
          <div class="urlbar">
            <select class="input" bind:value={ws.dev.previewUrl}>{#each ws.dev.urls as u (u)}<option value={u}>{u}</option>{/each}</select>
            <button class="icon-btn" title="Ricarica" aria-label="Ricarica" onclick={() => frame && (frame.src = ws.dev.previewUrl ?? "")}><Icon name="refresh" size={15} /></button>
          </div>
          {#if ws.dev.previewUrl}<iframe bind:this={frame} src={ws.dev.previewUrl} title="Anteprima"></iframe>{/if}
        {:else}
          <p class="hint">Nessun server acceso. Chiedi all'orchestratore di avviare il progetto: l'indirizzo locale comparirà qui.</p>
        {/if}
      {/if}
    </div>
    {#if ws.dev.processes.length || terms.length}
      <div class="bottom" class:open={bottomOpen && (termSel != null || proc)} class:isterm={termSel != null}>
        <div class="btabs">
          <button class="btoggle" onclick={() => (bottomOpen = !bottomOpen)} aria-label="Mostra o nascondi"><Icon name={bottomOpen ? "down" : "chevron"} size={12} /></button>
          {#each ws.dev.processes as p (p.id)}
            <button class="bt" class:on={proc?.id === p.id} onclick={() => { ws.dev.bottom = p.id; bottomOpen = true; }}>
              <span class="led" class:run={p.running} class:bad={!p.running && p.exitCode !== 0}></span>{p.name}
            </button>
          {/each}
          {#each terms as t (t.id)}
            <span class="bt tt" class:on={termSel === t.id}>
              <button class="ttn" onclick={() => { ws.dev.bottom = `term:${t.id}`; bottomOpen = true; }}><Icon name="terminal" size={11} />{t.name}{#if t.ended}<em> · chiuso</em>{/if}</button>
              <button class="ttx" onclick={() => chiudiTerminale(t.id)} aria-label="Chiudi {t.name}" title="Chiudi (ferma quello che ci gira)"><Icon name="x" size={10} /></button>
            </span>
          {/each}
          <button class="btoggle" onclick={nuovoTerminale} title="Nuovo terminale (⌃`)" aria-label="Nuovo terminale"><Icon name="plus" size={13} /></button>
        </div>
        {#if bottomOpen && proc}<pre class="log">{proc.log || "(nessun output)"}</pre>{/if}
        {#each terms as t (t.id)}
          <div class="tpane" hidden={!bottomOpen || termSel !== t.id}>
            <Terminal project={ws.path} active={bottomOpen && termSel === t.id} onexit={() => (t.ended = true)} />
          </div>
        {/each}
      </div>
    {/if}
  </section>

  <Orchestrator />
  {#if ws.quickOpen && studio.tab === ws}<QuickOpen />{/if}
  {#if menu}
    <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
    <div class="mscrim" onclick={() => (menu = null)} oncontextmenu={(e) => (e.preventDefault(), (menu = null))}></div>
    <div class="ctx" style="left:{menu.x}px; top:{menu.y}px" role="menu">
      {#if !menu.entry.dir}<button role="menuitem" onclick={() => { ws.openFile(menu!.entry.path); menu = null; }}>Apri</button>{/if}
      <button role="menuitem" onclick={() => chiedi("file", menu!.entry)}>Nuovo file…</button>
      <button role="menuitem" onclick={() => chiedi("dir", menu!.entry)}>Nuova cartella…</button>
      <hr />
      <button role="menuitem" onclick={() => chiedi("rename", menu!.entry)}>Rinomina…</button>
      <button role="menuitem" onclick={() => { navigator.clipboard?.writeText(rel(menu!.entry.path)); menu = null; }}>Copia percorso</button>
      <hr />
      <button role="menuitem" class="danger" onclick={() => { const p = menu!.entry.path; menu = null; ws.trashEntry(p); }}>Sposta nel Cestino</button>
    </div>
  {/if}
</div>

<style>
  .ws {
    position: relative;
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: var(--left) minmax(0, 1fr) var(--right);
  }
  .ws.dragging {
    cursor: col-resize;
    user-select: none;
  }
  /* Durante il trascinamento l'anteprima non deve rubare il mouse */
  .ws.dragging :global(iframe) {
    pointer-events: none;
  }
  .gutter {
    position: absolute;
    z-index: 5;
    top: 0;
    bottom: 0;
    width: 9px;
    cursor: col-resize;
    touch-action: none;
  }
  .gl {
    left: calc(var(--left) - 5px);
  }
  .gr {
    right: calc(var(--right) - 4px);
  }
  .gutter::after {
    content: "";
    position: absolute;
    top: 0;
    bottom: 0;
    left: 4px;
    width: 1px;
    background: transparent;
    transition: background-color 0.15s, box-shadow 0.15s;
  }
  .gutter:hover::after,
  .gutter:focus-visible::after,
  .gutter.on::after {
    width: 2px;
    left: 3.5px;
    background: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in oklch, var(--accent) 16%, transparent);
  }
  .gutter:focus-visible {
    outline: none;
  }
  .explorer {
    overflow: auto;
    padding: 6px 6px 16px;
    border-right: 1px solid var(--rule-soft);
    background:
      radial-gradient(260px 160px at 0% 0%, color-mix(in oklch, var(--accent) 7%, transparent), transparent 70%),
      var(--paper-2);
    scrollbar-width: thin;
    scrollbar-color: color-mix(in oklch, var(--accent) 25%, transparent) transparent;
  }
  .sec {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 12px 8px 4px;
    font-size: var(--t-xs);
    font-weight: 700;
    color: var(--ink-2);
  }
  .sec :global(svg) {
    color: var(--accent);
  }
  .tcount.live {
    color: var(--ok);
    background: color-mix(in oklch, var(--ok) 16%, transparent);
    animation: beat 1.1s ease-in-out infinite;
  }
  @keyframes beat {
    50% {
      opacity: 0.45;
    }
  }
  .tcount {
    min-width: 17px;
    height: 16px;
    padding: 0 5px;
    border-radius: 999px;
    display: grid;
    place-items: center;
    font-size: 0.62rem;
    font-weight: 750;
    color: var(--warn-ink);
    background: oklch(0.8 0.12 80 / 0.3);
  }
  .sec.side {
    gap: 2px;
    margin: 10px 4px 4px;
  }
  .sec.side button {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 24px;
    padding: 0 7px;
    border-radius: 6px;
    font-size: var(--t-xs);
    font-weight: 700;
    color: var(--ink-3);
  }
  .sec.side button.on {
    color: var(--ink);
    background: color-mix(in oklch, var(--accent) 10%, transparent);
  }
  .sec.side button:hover {
    color: var(--ink);
  }
  .sec.side .sp {
    flex: 1;
  }
  .naming {
    display: grid;
    gap: 4px;
    margin: 4px 6px 8px;
    padding: 8px;
    border-radius: 9px;
    background: var(--paper);
    box-shadow: inset 0 0 0 1.5px var(--accent);
  }
  .naming small {
    font-size: var(--t-xs);
    color: var(--ink-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .naming input {
    border: 0;
    outline: none;
    background: none;
    font-size: var(--t-sm);
    color: var(--ink);
    font-family: ui-monospace, Menlo, monospace;
  }
  .pcount {
    min-width: 15px;
    padding: 0 4px;
    border-radius: 999px;
    font-size: 0.6rem;
    font-weight: 750;
    color: var(--warn-ink);
    background: oklch(0.8 0.12 80 / 0.3);
  }
  .pcount.err {
    color: var(--danger);
    background: var(--danger-wash);
  }
  .probs {
    display: grid;
    gap: 2px;
    padding: 6px 4px;
  }
  .phead {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 4px 6px;
  }
  .phead small {
    flex: 1;
    min-width: 0;
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
  .phead .sm {
    height: 26px;
    padding: 0 8px;
    font-size: var(--t-xs);
    gap: 4px;
  }
  .pok {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 4px;
    font-size: var(--t-xs);
    color: var(--ok);
  }
  .prow {
    display: flex;
    align-items: flex-start;
    gap: 7px;
    padding: 5px 6px;
    border-radius: 7px;
    text-align: left;
    font-size: var(--t-xs);
  }
  .prow:hover {
    background: color-mix(in oklch, var(--accent) 7%, transparent);
  }
  .sev {
    flex: none;
    width: 8px;
    height: 8px;
    margin-top: 4px;
    border-radius: 50%;
  }
  .sev-error {
    background: var(--danger);
  }
  .sev-warning {
    background: var(--warn);
  }
  .pm {
    flex: 1;
    min-width: 0;
    color: var(--ink-2);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .pf {
    flex: none;
    color: var(--ink-3);
    font-family: ui-monospace, Menlo, monospace;
    font-size: 0.66rem;
  }
  .sb-prob {
    color: var(--danger);
    font-weight: 600;
    font-size: 0.7rem;
  }
  .status {
    flex: none;
    display: flex;
    align-items: center;
    gap: 14px;
    height: 24px;
    padding: 0 12px;
    border-top: 1px solid var(--rule-soft);
    background: var(--paper-2);
    font-size: 0.7rem;
    color: var(--ink-3);
    font-variant-numeric: tabular-nums;
  }
  .sp-path {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl;
    text-align: left;
  }
  .dirty-s {
    color: var(--accent);
    font-weight: 600;
  }
  .mscrim {
    position: fixed;
    inset: 0;
    z-index: 60;
  }
  .ctx {
    position: fixed;
    z-index: 61;
    min-width: 190px;
    padding: 5px;
    border-radius: 10px;
    background: var(--paper);
    box-shadow:
      0 0 0 1px var(--rule),
      0 12px 30px -10px oklch(0.2 0.06 var(--tint-h) / 0.35);
    display: grid;
  }
  .ctx button {
    padding: 6px 10px;
    border-radius: 6px;
    text-align: left;
    font-size: var(--t-sm);
  }
  .ctx button:hover {
    background: color-mix(in oklch, var(--accent) 10%, transparent);
  }
  .ctx .danger {
    color: var(--danger);
  }
  .ctx hr {
    border: 0;
    height: 1px;
    margin: 4px 2px;
    background: var(--rule-soft);
  }
  .center {
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .tabs {
    display: flex;
    gap: 1px;
    overflow-x: auto;
    background: var(--paper-2);
    border-bottom: 1px solid var(--rule-soft);
  }
  .tab {
    display: flex;
    align-items: center;
    flex: none;
    border-right: 1px solid var(--rule-soft);
    color: var(--ink-3);
  }
  .tab.on {
    background: var(--paper);
    color: var(--ink);
    box-shadow: inset 0 -2px 0 var(--accent);
  }
  .tname {
    display: flex;
    align-items: center;
    gap: 5px;
    height: 34px;
    padding: 0 10px 0 12px;
    font-size: var(--t-sm);
    white-space: nowrap;
  }
  .dirty {
    color: var(--accent);
    font-size: 0.7rem;
  }
  .tx {
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    margin-right: 6px;
    border-radius: 4px;
    color: var(--ink-3);
  }
  .tx:hover {
    background: var(--paper-2);
  }
  .pane {
    flex: 1;
    min-height: 0;
    overflow: auto;
    display: flex;
    flex-direction: column;
  }
  .pane > :global(.cm) {
    flex: 1;
  }
  .welcome {
    margin: auto;
    max-width: 380px;
    display: grid;
    justify-items: center;
    gap: 10px;
    text-align: center;
    color: var(--ink-3);
    font-size: var(--t-sm);
    line-height: 1.5;
  }
  .hint {
    padding: 18px;
    color: var(--ink-3);
    font-size: var(--t-sm);
  }
  .urlbar {
    display: flex;
    gap: 6px;
    padding: 8px;
  }
  iframe {
    flex: 1;
    width: 100%;
    border: 0;
    background: white;
  }
  .bottom {
    border-top: 1px solid var(--rule-soft);
    background: var(--paper-2);
  }
  .btabs {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 4px 8px;
  }
  .btoggle {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    color: var(--ink-3);
  }
  .tsp {
    flex: 1;
  }
  .tterm {
    flex: none;
    display: flex;
    align-items: center;
    gap: 5px;
    margin: 0 6px;
    padding: 0 9px;
    height: 26px;
    align-self: center;
    border-radius: 7px;
    font-size: var(--t-xs);
    font-weight: 600;
    color: var(--ink-3);
  }
  .tterm:hover {
    color: var(--accent);
    background: color-mix(in oklch, var(--accent) 9%, transparent);
  }
  .tpane {
    height: 260px;
    border-top: 1px solid var(--rule-soft);
  }
  .tt {
    display: flex;
    align-items: center;
    padding: 0 !important;
    overflow: hidden;
  }
  .ttn {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 3px 4px 3px 9px;
    font-size: var(--t-xs);
  }
  .ttn em {
    font-style: normal;
    color: var(--ink-3);
  }
  .ttx {
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    margin-right: 3px;
    border-radius: 50%;
    color: var(--ink-3);
  }
  .ttx:hover {
    color: var(--danger);
    background: var(--danger-wash);
  }
  .btabs .bt {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 3px 9px;
    border-radius: 999px;
    font-size: var(--t-xs);
    box-shadow: inset 0 0 0 1px var(--rule);
  }
  .btabs .bt.on {
    box-shadow: inset 0 0 0 1.5px var(--accent);
    color: var(--accent);
  }
  .led {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--ink-3);
  }
  .led.run {
    background: var(--ok);
  }
  .led.bad {
    background: var(--danger);
  }
  .log {
    margin: 0;
    height: 180px;
    overflow: auto;
    padding: 8px 12px;
    font-family: ui-monospace, Menlo, monospace;
    font-size: 11.5px;
    line-height: 1.5;
    white-space: pre-wrap;
    background: var(--paper);
  }
</style>
