<script lang="ts">
  // Oltremare Studio: la modalità sviluppo, in una finestra a sé.
  import { onMount } from "svelte";
  import Icon from "./lib/Icon.svelte";
  import Toasts from "./lib/Toasts.svelte";
  import ProjectPicker from "./lib/studio/ProjectPicker.svelte";
  import Workspace from "./lib/studio/Workspace.svelte";
  import RunBar from "./lib/studio/RunBar.svelte";
  import UsagePanel from "./lib/settings/UsagePanel.svelte";
  import { studio } from "./lib/state/studio.svelte";
  import { settings } from "./lib/state/settings.svelte";
  import { ui } from "./lib/state/ui.svelte";
  import { on } from "./lib/events";
  import { agents } from "./lib/state/agents.svelte";

  const q = new URLSearchParams(location.search);
  let prompt = $state(q.get("prompt") ?? "");
  let usage = $state(false);

  onMount(async () => {
    document.title = "Oltremare Studio";
    // In anteprima (fuori da Tauri) lo stato è raggiungibile dalla console per provare le schermate
    if (import.meta.env.DEV) (window as unknown as { studio: typeof studio }).studio = studio;
    await settings.load().catch(() => {});
    await Promise.all([studio.loadProjects(), studio.loadClaudex(), agents.load().catch(() => {})]);
    // Le schede dell'ultima volta, poi il progetto chiesto da chi ha aperto la finestra
    studio.restoreTabs();
    const p = q.get("project");
    if (p) studio.open(p, { prompt });
    on("studio-open", (e) => {
      if (e.project) studio.open(e.project, { prompt: e.prompt ?? undefined });
      else if (e.prompt && studio.tab) studio.tab.draft = e.prompt;
    });
  });

  // I profili di Claudex possono cambiare mentre lo Studio è aperto: si rileggono quando la finestra torna davanti
  function focus() {
    studio.loadClaudex();
  }

  function key(e: KeyboardEvent) {
    const mod = e.metaKey || e.ctrlKey;
    if (!mod) return;
    if (e.key.toLowerCase() === "p" && !e.shiftKey) {
      e.preventDefault();
      studio.picker();
    } else if (e.shiftKey && (e.key === "]" || e.key === "}")) {
      e.preventDefault();
      studio.cycle(1);
    } else if (e.shiftKey && (e.key === "[" || e.key === "{")) {
      e.preventDefault();
      studio.cycle(-1);
    } else if (/^[1-9]$/.test(e.key) && !e.shiftKey && !e.altKey) {
      const ws = studio.tabs[Number(e.key) - 1];
      if (ws) {
        e.preventDefault();
        studio.show(ws);
      }
    }
  }
</script>

<svelte:window onkeydown={key} onfocus={focus} />

<div class="studio" class:mac={ui.isMac}>
  <header class="bar" data-tauri-drag-region>
    <span class="brand" data-tauri-drag-region><span class="dot"></span><em>Oltremare</em> Studio</span>
    {#if studio.tabs.length}
      <nav class="tabs" aria-label="Progetti aperti">
        {#each studio.tabs as t, i (t.id)}
          {@const on = studio.tab === t}
          {@const lavora = t.busy}
          <div class="ptab" class:on class:lavora style="--h:{t.project.hue}">
            <button class="pt" onclick={() => studio.show(t)} title="{t.project.path}{i < 9 ? ` · ⌘${i + 1}` : ''}">
              <span class="pdot"></span>
              <span class="pn">{t.project.name}</span>
              {#if lavora}<span class="pulse" title="Claude sta lavorando"></span>{/if}
            </button>
            <button class="px" onclick={() => studio.closeTab(t)} aria-label="Chiudi {t.project.name}" title="Chiudi la scheda"><Icon name="x" size={11} /></button>
          </div>
        {/each}
      </nav>
      <button class="padd" class:on={!studio.tab} onclick={() => studio.picker()} title="Apri un altro progetto (⌘P)" aria-label="Apri un altro progetto"><Icon name="plus" size={14} /></button>
    {/if}
    {#if studio.tab?.git.state?.repo}
      {@const g = studio.tab.git.state}
      <span class="branch" data-tauri-drag-region title={g.upstream ? `Remoto: ${g.upstream}` : "Nessun ramo remoto"}>
        <Icon name="branch" size={12} /> {g.branch}
        {#if g.ahead}<span class="ab">↑{g.ahead}</span>{/if}{#if g.behind}<span class="ab">↓{g.behind}</span>{/if}
      </span>
    {/if}
    <span class="sp" data-tauri-drag-region></span>
    {#if studio.tab}{#key studio.tab.id}<RunBar ws={studio.tab} />{/key}{/if}
    <button class="cons" class:on={usage} onclick={() => (usage = !usage)} title="Consumi di Claude, GPT e delle altre AI"><Icon name="bolt" size={13} /> Consumi</button>
  </header>
  {#if usage}
    <div class="scrim" role="presentation" onclick={() => (usage = false)}></div>
    <aside class="drawer" aria-label="Consumi">
      <header class="dh"><strong>Consumi</strong><button class="icon-btn" onclick={() => (usage = false)} aria-label="Chiudi"><Icon name="x" size={15} /></button></header>
      <div class="db">
        {#key usage}<UsagePanel />{/key}
      </div>
    </aside>
  {/if}

  {#each studio.tabs as t (t.id)}
    <div class="host" hidden={studio.tab !== t}><Workspace ws={t} /></div>
  {/each}
  {#if !studio.tab}
    <ProjectPicker />
  {/if}
  <Toasts />
</div>

<style>
  .studio {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--paper);
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 42px;
    padding: 0 12px 0 16px;
    border-bottom: 1px solid var(--rule-soft);
    background: var(--paper-2);
    flex: none;
  }
  .studio.mac .bar {
    padding-left: 86px;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 7px;
    font-size: var(--t-sm);
    font-weight: 650;
    color: var(--ink-2);
  }
  .brand em {
    font-family: var(--font-serif);
    font-size: 1.15rem;
    font-weight: 400;
    color: var(--accent);
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent);
  }
  .tabs {
    display: flex;
    align-items: center;
    gap: 2px;
    margin-left: 10px;
    min-width: 0;
    overflow-x: auto;
    scrollbar-width: none;
  }
  .ptab {
    position: relative;
    display: flex;
    align-items: center;
    flex: none;
    max-width: 200px;
    height: 30px;
    border-radius: 9px;
    color: var(--ink-3);
    transition: background-color 0.15s, color 0.15s;
  }
  .ptab:hover {
    background: var(--paper);
    color: var(--ink-2);
  }
  .ptab.on {
    background: var(--paper);
    color: var(--ink);
    box-shadow: 0 1px 2px oklch(0.3 0.05 var(--tint-h) / 0.08), inset 0 -2px 0 oklch(0.62 0.16 var(--h));
  }
  .pt {
    display: flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
    height: 100%;
    padding: 0 4px 0 10px;
    font-size: var(--t-sm);
    font-weight: 600;
  }
  .pdot {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: 3px;
    background: oklch(0.72 0.15 var(--h));
  }
  .pn {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .pulse {
    flex: none;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--accent);
    animation: beat 1.1s ease-in-out infinite;
  }
  @keyframes beat {
    50% {
      transform: scale(0.55);
      opacity: 0.45;
    }
  }
  .px {
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    margin: 0 5px 0 0;
    border-radius: 6px;
    color: var(--ink-3);
    opacity: 0;
    transition: opacity 0.12s, background-color 0.12s;
  }
  .ptab:hover .px,
  .ptab.on .px,
  .px:focus-visible {
    opacity: 1;
  }
  .px:hover {
    color: var(--danger);
    background: var(--danger-wash);
  }
  .padd {
    display: grid;
    place-items: center;
    flex: none;
    width: 28px;
    height: 28px;
    border-radius: 8px;
    color: var(--ink-3);
  }
  .padd:hover,
  .padd.on {
    color: var(--accent);
    background: color-mix(in oklch, var(--accent) 10%, transparent);
  }
  .host {
    display: contents;
  }
  .host[hidden] {
    display: none;
  }
  .branch {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: var(--t-xs);
    color: var(--accent);
  }
  .ab {
    padding: 0 5px;
    border-radius: 999px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    background: color-mix(in oklch, var(--accent) 12%, transparent);
  }
  .sp {
    flex: 1;
    height: 100%;
  }
  .cons {
    display: flex;
    align-items: center;
    gap: 5px;
    height: 28px;
    padding: 0 11px;
    border-radius: 999px;
    font-size: var(--t-xs);
    font-weight: 650;
    color: oklch(0.5 0.14 80);
    background: oklch(0.82 0.12 85 / 0.25);
  }
  .cons.on,
  .cons:hover {
    background: oklch(0.82 0.12 85 / 0.4);
  }
  :global([data-theme="dark"]) .cons {
    color: oklch(0.86 0.12 85);
    background: oklch(0.6 0.12 85 / 0.22);
  }
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 50;
    background: var(--scrim);
  }
  .drawer {
    position: fixed;
    z-index: 51;
    top: 0;
    right: 0;
    bottom: 0;
    width: min(520px, 100vw);
    display: flex;
    flex-direction: column;
    background: var(--paper);
    box-shadow: var(--shadow-sheet);
    animation: slide 0.28s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  @keyframes slide {
    from {
      transform: translateX(30px);
      opacity: 0;
    }
  }
  .dh {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 40px 20px 10px;
  }
  .dh strong {
    font-family: var(--font-serif);
    font-weight: 400;
    font-size: 1.7rem;
  }
  .db {
    flex: 1;
    overflow-y: auto;
    padding: 0 20px 24px;
    display: grid;
    align-content: start;
    gap: 12px;
  }
</style>
