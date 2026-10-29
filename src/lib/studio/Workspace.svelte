<script lang="ts">
  // Lo spazio di lavoro dello Studio: esplora file, editor a schede, differenze, anteprima, processi, orchestratore.
  import Icon from "../Icon.svelte";
  import FileTree from "../dev/FileTree.svelte";
  import CodeEditor from "./CodeEditor.svelte";
  import Orchestrator from "./Orchestrator.svelte";
  import IssuesPanel from "./IssuesPanel.svelte";
  import SourceControl from "./SourceControl.svelte";
  import DiffViewer from "./DiffViewer.svelte";
  import { git } from "../state/git.svelte";
  import { dev } from "../state/dev.svelte";
  import { studio } from "../state/studio.svelte";

  let { prompt = "" }: { prompt?: string } = $props();
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
  let ws: HTMLElement | undefined = $state();
  let dragging = $state<"left" | "right" | null>(null);

  function imposta(k: "left" | "right", v: number) {
    // Il centro tiene almeno 360 px
    const spazio = (ws?.clientWidth ?? 1400) - (k === "left" ? right : left) - 360;
    const n = Math.round(Math.min(Math.max(v, LIM[k][0]), LIM[k][1], Math.max(spazio, LIM[k][0])));
    if (k === "left") left = n;
    else right = n;
    try {
      localStorage.setItem(`oltremare:studio-${k}`, String(n));
    } catch {}
  }

  function trascina(k: "left" | "right", e: PointerEvent) {
    if (e.button !== 0 || !ws) return;
    e.preventDefault();
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
    dragging = k;
    const box = ws.getBoundingClientRect();
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
  const proc = $derived(dev.processes.find((p) => p.id === dev.bottom) ?? dev.processes[0] ?? null);
</script>

<div class="ws" bind:this={ws} class:dragging style="--left:{left}px; --right:{right}px">
  <!-- Separatore che si sposta (ruolo ARIA «separator» con valore): è un controllo, si può mettere a fuoco -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
  <div class="gutter gl" class:on={dragging === "left"} role="separator" aria-orientation="vertical" aria-label="Larghezza dell'esplora file" aria-valuenow={left} aria-valuemin={LIM.left[0]} aria-valuemax={LIM.left[1]} tabindex="0" title="Trascina per allargare · doppio clic per tornare com'era" onpointerdown={(e) => trascina("left", e)} ondblclick={() => imposta("left", DEF.left)} onkeydown={(e) => tasti("left", e)}></div>
  <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
  <div class="gutter gr" class:on={dragging === "right"} role="separator" aria-orientation="vertical" aria-label="Larghezza della chat" aria-valuenow={right} aria-valuemin={LIM.right[0]} aria-valuemax={LIM.right[1]} tabindex="0" title="Trascina per allargare · doppio clic per tornare com'era" onpointerdown={(e) => trascina("right", e)} ondblclick={() => imposta("right", DEF.right)} onkeydown={(e) => tasti("right", e)}></div>
  <aside class="explorer">
    <IssuesPanel />
    <SourceControl />
    <div class="sec"><Icon name="folder" size={12} /> {studio.project?.name ?? "Esplora"}</div>
    {#if dev.folder}<FileTree path={dev.folder} onopen={(p) => studio.openFile(p)} active={dev.tab === "file" ? studio.active : null} />{/if}
  </aside>

  <section class="center">
    <div class="tabs">
      {#each studio.files as f (f.path)}
        <div class="tab" class:on={dev.tab === "file" && studio.active === f.path}>
          <button class="tname" onclick={() => { studio.active = f.path; dev.tab = "file"; }} title={f.path}>{f.name}{#if f.content !== f.saved}<span class="dirty">●</span>{/if}</button>
          <button class="tx" onclick={() => studio.closeFile(f.path)} aria-label="Chiudi {f.name}"><Icon name="x" size={11} /></button>
        </div>
      {/each}
      <div class="tab" class:on={dev.tab === "diff"}><button class="tname" onclick={() => git.showDiff(git.diffFile, git.diffStaged)}><Icon name="layers" size={12} /> Differenze{#if git.changed.length + git.staged.length}<span class="tcount">{git.changed.length + git.staged.length}</span>{/if}</button></div>
      <div class="tab" class:on={dev.tab === "preview"}><button class="tname" onclick={() => (dev.tab = "preview")}><Icon name="globe" size={12} /> Anteprima{dev.urls.length ? ` · ${dev.urls.length}` : ""}</button></div>
    </div>
    <div class="pane">
      {#if dev.tab === "file"}
        {#if studio.current}
          {#if studio.current.binary}
            <p class="hint">File binario: non si può modificare qui.</p>
          {:else}
            {#key studio.current.path}
              <CodeEditor path={studio.current.path} value={studio.current.content} onchange={(v) => studio.current && (studio.current.content = v)} onsave={() => studio.save()} />
            {/key}
          {/if}
        {:else}
          <div class="welcome">
            <Icon name="draft" size={26} />
            <p>Apri un file dall'esplora risorse, oppure chiedi all'orchestratore cosa fare. I file modificati da Claude compaiono in «Modifiche», pronti per il commit.</p>
          </div>
        {/if}
      {:else if dev.tab === "diff"}
        <DiffViewer />
      {:else}
        {#if dev.urls.length}
          <div class="urlbar">
            <select class="input" bind:value={dev.previewUrl}>{#each dev.urls as u (u)}<option value={u}>{u}</option>{/each}</select>
            <button class="icon-btn" title="Ricarica" aria-label="Ricarica" onclick={() => frame && (frame.src = dev.previewUrl ?? "")}><Icon name="refresh" size={15} /></button>
          </div>
          {#if dev.previewUrl}<iframe bind:this={frame} src={dev.previewUrl} title="Anteprima"></iframe>{/if}
        {:else}
          <p class="hint">Nessun server acceso. Chiedi all'orchestratore di avviare il progetto: l'indirizzo locale comparirà qui.</p>
        {/if}
      {/if}
    </div>
    {#if dev.processes.length}
      <div class="bottom">
        <div class="btabs">
          <button class="btoggle" onclick={() => (bottomOpen = !bottomOpen)} aria-label="Mostra o nascondi"><Icon name={bottomOpen ? "down" : "chevron"} size={12} /></button>
          {#each dev.processes as p (p.id)}
            <button class:on={proc?.id === p.id} onclick={() => { dev.bottom = p.id; bottomOpen = true; }}>
              <span class="led" class:run={p.running} class:bad={!p.running && p.exitCode !== 0}></span>{p.name}
            </button>
          {/each}
        </div>
        {#if bottomOpen && proc}<pre class="log">{proc.log || "(nessun output)"}</pre>{/if}
      </div>
    {/if}
  </section>

  <Orchestrator {prompt} />
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
  .btabs button:not(.btoggle) {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 3px 9px;
    border-radius: 999px;
    font-size: var(--t-xs);
    box-shadow: inset 0 0 0 1px var(--rule);
  }
  .btabs button.on {
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
