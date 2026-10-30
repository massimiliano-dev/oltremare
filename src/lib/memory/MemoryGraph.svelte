<script lang="ts">
  // La memoria vista come grafo: una nota per nodo, un collegamento [[...]] per filo, colore per cartella.
  // Disegno su canvas con una simulazione a forze; si trascina, si ingrandisce, si cerca, si legge una nota a lato.
  import { onMount } from "svelte";
  import Icon from "../Icon.svelte";
  import { markdown } from "../markdown";
  import { api, errorText, type MemoryGraph, type MemoryNote } from "../api";
  import { ui } from "../state/ui.svelte";

  let { focus = null, compact = false }: { focus?: string | null; compact?: boolean } = $props();

  type Node = { id: string; title: string; folder: string; size: number; missing: boolean; x: number; y: number; vx: number; vy: number; deg: number; r: number; fixed?: boolean };
  type Edge = { a: Node; b: Node };

  let canvas: HTMLCanvasElement;
  let wrap: HTMLDivElement;
  let nodes: Node[] = [];
  let edges: Edge[] = [];
  // svelte-ignore non_reactive_update
  let byId = new Map<string, Node>();
  let count = $state({ nodes: 0, edges: 0 });
  let loading = $state(true);
  let error = $state("");
  let query = $state("");
  let hidden = $state<Record<string, boolean>>({});
  let selected = $state<string | null>(null);
  let hovered = $state<string | null>(null);
  let note = $state<MemoryNote | null>(null);
  let folders = $state<string[]>([]);

  // Vista: scala e scorrimento
  let scale = 1;
  let tx = 0;
  let ty = 0;
  let alpha = 1;
  let raf = 0;
  let dragging: Node | null = null;
  let panning = false;
  let last = { x: 0, y: 0 };
  let dpr = 1;

  const HUE: Record<string, number> = { Persone: 35, Progetti: 262, Argomenti: 150, Diario: 200, "": 285 };
  const hue = (f: string) => HUE[f] ?? 320;
  const dark = () => document.documentElement.dataset.theme === "dark";
  const col = (f: string, a = 1) => `oklch(${dark() ? 0.72 : 0.58} 0.15 ${hue(f)} / ${a})`;

  async function load() {
    loading = true;
    error = "";
    try {
      const g: MemoryGraph = await api.memoryGraph();
      const n = g.nodes.length;
      byId = new Map();
      nodes = g.nodes.map((v, i) => {
        const ang = (i / Math.max(1, n)) * Math.PI * 2;
        const rad = 80 + Math.sqrt(n) * 30 * (0.4 + ((i * 7919) % 100) / 100);
        return { id: v.id, title: v.title, folder: v.folder, size: v.size, missing: !!v.missing, x: Math.cos(ang) * rad, y: Math.sin(ang) * rad, vx: 0, vy: 0, deg: 0, r: 4 };
      });
      for (const nd of nodes) byId.set(nd.id, nd);
      edges = g.edges.map((e) => ({ a: byId.get(e.from)!, b: byId.get(e.to)! })).filter((e) => e.a && e.b);
      for (const e of edges) {
        e.a.deg++;
        e.b.deg++;
      }
      for (const nd of nodes) nd.r = nd.missing ? 3 : 4 + Math.min(10, Math.sqrt(nd.deg) * 2.2) + Math.min(3, nd.size / 4000);
      folders = [...new Set(nodes.map((x) => x.folder))].sort((a, b) => (a === "" ? 1 : b === "" ? -1 : a.localeCompare(b)));
      count = { nodes: nodes.length, edges: edges.length };
      // Qualche centinaio di passi prima del primo disegno, così il grafo nasce già assestato
      alpha = 1;
      for (let i = 0; i < 260 && alpha > 0.004; i++) step();
      alpha = 0.3;
      fit();
      if (focus) focusOn(focus);
    } catch (e) {
      error = errorText(e);
    } finally {
      loading = false;
    }
  }

  function visible(nd: Node) {
    return !hidden[nd.folder];
  }

  function matches(nd: Node) {
    const q = query.trim().toLowerCase();
    return !q || nd.title.toLowerCase().includes(q) || nd.folder.toLowerCase().includes(q);
  }

  function neighbors(id: string | null) {
    const s = new Set<string>();
    if (!id) return s;
    for (const e of edges) {
      if (e.a.id === id) s.add(e.b.id);
      if (e.b.id === id) s.add(e.a.id);
    }
    return s;
  }

  /** Un passo della simulazione: repulsione tra nodi, molle sui fili, richiamo verso il centro */
  function step() {
    if (alpha < 0.004) return;
    const vis = nodes.filter(visible);
    const k = 1400;
    for (let i = 0; i < vis.length; i++) {
      const a = vis[i];
      for (let j = i + 1; j < vis.length; j++) {
        const b = vis[j];
        let dx = a.x - b.x;
        let dy = a.y - b.y;
        let d2 = dx * dx + dy * dy;
        if (d2 < 1) {
          dx = (Math.random() - 0.5) * 2;
          dy = (Math.random() - 0.5) * 2;
          d2 = dx * dx + dy * dy + 1;
        }
        if (d2 > 90000) continue;
        const f = (k / d2) * alpha;
        a.vx += dx * f;
        a.vy += dy * f;
        b.vx -= dx * f;
        b.vy -= dy * f;
      }
    }
    for (const e of edges) {
      if (!visible(e.a) || !visible(e.b)) continue;
      const dx = e.b.x - e.a.x;
      const dy = e.b.y - e.a.y;
      const d = Math.sqrt(dx * dx + dy * dy) || 1;
      const rest = 70 + (e.a.r + e.b.r) * 2;
      const f = ((d - rest) / d) * 0.04 * alpha;
      e.a.vx += dx * f;
      e.a.vy += dy * f;
      e.b.vx -= dx * f;
      e.b.vy -= dy * f;
    }
    for (const nd of vis) {
      nd.vx -= nd.x * 0.003 * alpha;
      nd.vy -= nd.y * 0.003 * alpha;
      if (nd.fixed) {
        nd.vx = nd.vy = 0;
        continue;
      }
      nd.vx *= 0.82;
      nd.vy *= 0.82;
      nd.x += nd.vx;
      nd.y += nd.vy;
    }
    alpha *= 0.985;
  }

  function fit() {
    const w = canvas?.clientWidth || 600;
    const h = canvas?.clientHeight || 400;
    const vis = nodes.filter(visible);
    if (!vis.length) return;
    const xs = vis.map((n) => n.x);
    const ys = vis.map((n) => n.y);
    const bw = Math.max(...xs) - Math.min(...xs) + 120;
    const bh = Math.max(...ys) - Math.min(...ys) + 120;
    scale = Math.min(2, Math.max(0.25, Math.min(w / bw, h / bh)));
    tx = w / 2 - ((Math.max(...xs) + Math.min(...xs)) / 2) * scale;
    ty = h / 2 - ((Math.max(...ys) + Math.min(...ys)) / 2) * scale;
  }

  function draw() {
    const ctx = canvas?.getContext("2d");
    if (!ctx) return;
    const w = canvas.clientWidth;
    const h = canvas.clientHeight;
    if (canvas.width !== Math.round(w * dpr) || canvas.height !== Math.round(h * dpr)) {
      canvas.width = Math.round(w * dpr);
      canvas.height = Math.round(h * dpr);
    }
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);
    ctx.save();
    ctx.translate(tx, ty);
    ctx.scale(scale, scale);
    const st = getComputedStyle(document.documentElement);
    const ink = st.getPropertyValue("--ink").trim() || "#222";
    const faint = st.getPropertyValue("--rule").trim() || "#ccc";
    const active = selected ?? hovered;
    const near = neighbors(active);
    const q = query.trim();
    // Fili
    ctx.lineWidth = 1 / scale;
    for (const e of edges) {
      if (!visible(e.a) || !visible(e.b)) continue;
      const lit = active && (e.a.id === active || e.b.id === active);
      ctx.strokeStyle = lit ? col(e.a.id === active ? e.b.folder : e.a.folder, 0.9) : faint;
      ctx.globalAlpha = active && !lit ? 0.25 : 1;
      ctx.lineWidth = (lit ? 1.8 : 1) / scale;
      ctx.beginPath();
      ctx.moveTo(e.a.x, e.a.y);
      ctx.lineTo(e.b.x, e.b.y);
      ctx.stroke();
    }
    // Nodi
    for (const nd of nodes) {
      if (!visible(nd)) continue;
      const lit = !active || nd.id === active || near.has(nd.id);
      const hit = !q || matches(nd);
      ctx.globalAlpha = lit && hit ? 1 : 0.18;
      ctx.beginPath();
      ctx.arc(nd.x, nd.y, nd.r, 0, Math.PI * 2);
      if (nd.missing) {
        ctx.strokeStyle = col("", 0.9);
        ctx.lineWidth = 1.2 / scale;
        ctx.setLineDash([2 / scale, 2 / scale]);
        ctx.stroke();
        ctx.setLineDash([]);
      } else {
        ctx.fillStyle = col(nd.folder);
        ctx.fill();
      }
      if (nd.id === selected) {
        ctx.strokeStyle = ink;
        ctx.lineWidth = 2 / scale;
        ctx.beginPath();
        ctx.arc(nd.x, nd.y, nd.r + 3 / scale, 0, Math.PI * 2);
        ctx.stroke();
      }
    }
    // Etichette: sempre per i nodi importanti e per chi è a fuoco; tutte quando si ingrandisce
    ctx.globalAlpha = 1;
    ctx.textAlign = "center";
    ctx.textBaseline = "top";
    const fs = Math.max(9, 11 / scale);
    ctx.font = `500 ${fs}px ui-sans-serif, -apple-system, system-ui, sans-serif`;
    for (const nd of nodes) {
      if (!visible(nd)) continue;
      const important = nd.deg >= 3 || scale > 1.3 || nd.id === active || near.has(nd.id) || (q && matches(nd));
      if (!important) continue;
      const lit = !active || nd.id === active || near.has(nd.id);
      ctx.globalAlpha = lit ? (nd.id === active ? 1 : 0.85) : 0.25;
      ctx.fillStyle = ink;
      ctx.fillText(nd.title, nd.x, nd.y + nd.r + 3 / scale);
    }
    ctx.restore();
  }

  function loop() {
    step();
    draw();
    raf = requestAnimationFrame(loop);
  }

  function toWorld(cx: number, cy: number) {
    const b = canvas.getBoundingClientRect();
    return { x: (cx - b.left - tx) / scale, y: (cy - b.top - ty) / scale };
  }

  function at(cx: number, cy: number): Node | null {
    const p = toWorld(cx, cy);
    let best: Node | null = null;
    let bd = Infinity;
    for (const nd of nodes) {
      if (!visible(nd)) continue;
      const d = Math.hypot(nd.x - p.x, nd.y - p.y);
      if (d < nd.r + 6 / scale && d < bd) {
        bd = d;
        best = nd;
      }
    }
    return best;
  }

  function down(e: PointerEvent) {
    canvas.setPointerCapture(e.pointerId);
    last = { x: e.clientX, y: e.clientY };
    const nd = at(e.clientX, e.clientY);
    if (nd) {
      dragging = nd;
      nd.fixed = true;
    } else panning = true;
  }
  function move(e: PointerEvent) {
    if (dragging) {
      const p = toWorld(e.clientX, e.clientY);
      dragging.x = p.x;
      dragging.y = p.y;
      alpha = Math.max(alpha, 0.3);
    } else if (panning) {
      tx += e.clientX - last.x;
      ty += e.clientY - last.y;
      last = { x: e.clientX, y: e.clientY };
    } else {
      const nd = at(e.clientX, e.clientY);
      hovered = nd?.id ?? null;
      canvas.style.cursor = nd ? "pointer" : "grab";
    }
  }
  function up(e: PointerEvent) {
    const moved = Math.hypot(e.clientX - last.x, e.clientY - last.y) > 3;
    if (dragging) {
      dragging.fixed = false;
      if (!moved) select(dragging.id);
      dragging = null;
    } else if (panning && !moved) {
      selected = null;
      note = null;
    }
    panning = false;
  }
  function wheel(e: WheelEvent) {
    e.preventDefault();
    const b = canvas.getBoundingClientRect();
    const mx = e.clientX - b.left;
    const my = e.clientY - b.top;
    const f = Math.exp(-e.deltaY * 0.0015);
    const ns = Math.min(5, Math.max(0.15, scale * f));
    tx = mx - ((mx - tx) * ns) / scale;
    ty = my - ((my - ty) * ns) / scale;
    scale = ns;
  }

  async function select(id: string) {
    selected = id;
    const nd = byId.get(id);
    if (!nd || nd.missing) {
      note = null;
      return;
    }
    try {
      note = await api.memoryRead(nd.id.replace(/\.md$/, ""));
    } catch (e) {
      ui.toast(errorText(e), "error");
    }
  }

  function center(id: string) {
    const nd = byId.get(id);
    if (!nd) return;
    tx = canvas.clientWidth / 2 - nd.x * scale;
    ty = canvas.clientHeight / 2 - nd.y * scale;
  }

  /** I [[collegamenti]] nel testo diventano cliccabili */
  function render(t: string) {
    return markdown(t.replace(/^---\n[\s\S]*?\n---\n/, "")).replace(/\[\[([^\]|]+)(?:\|([^\]]+))?\]\]/g, (_, a: string, b?: string) => `<a class="wl" data-nota="${a.trim()}">${(b ?? a).trim()}</a>`);
  }

  function clickNote(e: MouseEvent) {
    const a = (e.target as HTMLElement).closest("a.wl") as HTMLElement | null;
    if (!a) return;
    const t = a.dataset.nota ?? "";
    const nd = nodes.find((n) => n.title.toLowerCase() === t.toLowerCase());
    if (nd) {
      select(nd.id);
      center(nd.id);
    }
  }

  onMount(() => {
    dpr = window.devicePixelRatio || 1;
    load();
    raf = requestAnimationFrame(loop);
    const ro = new ResizeObserver(() => draw());
    ro.observe(wrap);
    return () => {
      cancelAnimationFrame(raf);
      ro.disconnect();
    };
  });

  function focusOn(f: string) {
    const nd = byId.get(f) ?? nodes.find((n) => f.endsWith(`/${n.id}`));
    if (nd) {
      select(nd.id);
      center(nd.id);
    }
  }

  $effect(() => {
    if (focus && nodes.length) focusOn(focus);
  });
</script>

<div class="mg" class:compact>
  <div class="gwrap" bind:this={wrap}>
    <div class="bar">
      <label class="search"><Icon name="search" size={14} /><input placeholder="Cerca una nota…" bind:value={query} aria-label="Cerca nella memoria" /></label>
      <div class="chips">
        {#each folders as f (f)}
          <button class="chip" class:off={hidden[f]} style="--h:{hue(f)}" onclick={() => (hidden[f] = !hidden[f])} aria-pressed={!hidden[f]}>{f || "principali"}</button>
        {/each}
      </div>
      <span class="count">{count.nodes} note · {count.edges} collegamenti</span>
      <button class="icon-btn" onclick={() => { alpha = 1; fit(); }} title="Ridisponi e adatta" aria-label="Adatta"><Icon name="refresh" size={14} /></button>
      <button class="icon-btn" onclick={() => api.memoryOpen().catch((e) => ui.toast(errorText(e), "error"))} title="Apri in Obsidian" aria-label="Apri in Obsidian"><Icon name="book" size={14} /></button>
    </div>
    {#if loading}
      <p class="msg">Leggo la memoria…</p>
    {:else if error}
      <p class="msg">{error}</p>
    {:else if !count.nodes}
      <p class="msg">La memoria è vuota: le note compaiono qui man mano che gli agenti (o tu) le scrivono.</p>
    {/if}
    <canvas bind:this={canvas} onpointerdown={down} onpointermove={move} onpointerup={up} onpointercancel={up} onwheel={wheel} aria-label="Grafo delle note"></canvas>
  </div>
  <aside class="side" class:open={!!selected}>
    {#if note}
      <header>
        <span class="chip" style="--h:{hue(note.percorso.includes('/') ? note.percorso.split('/')[0] : '')}">{note.percorso.includes("/") ? note.percorso.split("/")[0] : "principale"}</span>
        <h3>{note.nota}</h3>
        <button class="icon-btn" onclick={() => api.memoryOpen(note!.fullPath).catch((e) => ui.toast(errorText(e), "error"))} title="Apri in Obsidian" aria-label="Apri in Obsidian"><Icon name="external" size={14} /></button>
        <button class="icon-btn" onclick={() => { selected = null; note = null; }} aria-label="Chiudi"><Icon name="x" size={14} /></button>
      </header>
      <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
      <div class="md text" onclick={clickNote}>{@html render(note.testo)}</div>
      {#if note.citata_da.length}
        <div class="back">
          <strong>Citata da</strong>
          {#each note.citata_da as c (c)}
            <button class="link" onclick={() => { const nd = nodes.find((n) => n.title === c); if (nd) { select(nd.id); center(nd.id); } }}>{c}</button>
          {/each}
        </div>
      {/if}
    {:else if selected}
      <p class="msg">Questa nota non esiste ancora: qualcuno la cita con [[{byId.get(selected)?.title}]]. Quando verrà scritta comparirà qui.</p>
    {:else}
      <p class="msg">Clicca una nota per leggerla. Trascina per spostare, rotella per ingrandire.</p>
    {/if}
  </aside>
</div>

<style>
  .mg {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr) 340px;
    background: var(--paper);
  }
  .mg.compact {
    grid-template-columns: minmax(0, 1fr) 300px;
  }
  .gwrap {
    position: relative;
    min-width: 0;
    min-height: 0;
  }
  canvas {
    width: 100%;
    height: 100%;
    display: block;
    cursor: grab;
    touch-action: none;
    background:
      radial-gradient(circle at 50% 40%, color-mix(in oklch, var(--accent) 5%, transparent), transparent 60%),
      var(--paper);
  }
  .bar {
    position: absolute;
    z-index: 2;
    top: 10px;
    left: 10px;
    right: 10px;
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    pointer-events: none;
  }
  .bar > * {
    pointer-events: auto;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 30px;
    padding: 0 10px;
    border-radius: 999px;
    color: var(--ink-3);
    background: color-mix(in oklch, var(--paper) 88%, transparent);
    backdrop-filter: blur(8px);
    box-shadow: inset 0 0 0 1px var(--rule);
  }
  .search input {
    width: 170px;
    border: 0;
    background: none;
    outline: none;
    font-size: var(--t-sm);
    color: var(--ink);
  }
  .chips {
    display: flex;
    gap: 4px;
    flex-wrap: wrap;
  }
  .chip {
    padding: 3px 10px;
    border-radius: 999px;
    font-size: 0.7rem;
    font-weight: 700;
    color: oklch(0.42 0.14 var(--h));
    background: oklch(0.75 0.13 var(--h) / 0.25);
    transition: opacity 0.15s;
  }
  :global([data-theme="dark"]) .chip {
    color: oklch(0.86 0.11 var(--h));
  }
  .chip.off {
    opacity: 0.35;
    text-decoration: line-through;
  }
  .count {
    margin-left: auto;
    font-size: var(--t-xs);
    color: var(--ink-3);
    padding: 4px 8px;
    border-radius: 999px;
    background: color-mix(in oklch, var(--paper) 88%, transparent);
  }
  .msg {
    position: absolute;
    z-index: 1;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    margin: 0;
    max-width: 360px;
    text-align: center;
    font-size: var(--t-sm);
    color: var(--ink-3);
    line-height: 1.5;
  }
  .side {
    min-height: 0;
    overflow-y: auto;
    padding: 14px 16px 20px;
    border-left: 1px solid var(--rule-soft);
    background: var(--paper-2);
    display: grid;
    align-content: start;
    gap: 12px;
  }
  .side .msg {
    position: static;
    transform: none;
  }
  .side header {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }
  .side h3 {
    flex: 1;
    min-width: 0;
    margin: 0;
    font-family: var(--font-serif);
    font-weight: 400;
    font-size: 1.25rem;
    line-height: 1.2;
  }
  .text {
    font-size: var(--t-sm);
  }
  .text :global(a.wl) {
    color: var(--accent);
    text-decoration: underline;
    text-decoration-color: color-mix(in oklch, var(--accent) 40%, transparent);
    cursor: pointer;
  }
  .back {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
    font-size: var(--t-xs);
  }
  .back strong {
    color: var(--ink-2);
  }
  @media (max-width: 900px) {
    .mg,
    .mg.compact {
      grid-template-columns: 1fr;
    }
    .side {
      border-left: 0;
      border-top: 1px solid var(--rule-soft);
      max-height: 45%;
    }
  }
</style>
