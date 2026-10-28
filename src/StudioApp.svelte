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
  import { dev } from "./lib/state/dev.svelte";
  import { settings } from "./lib/state/settings.svelte";
  import { ui } from "./lib/state/ui.svelte";
  import { on } from "./lib/events";
  import { agents } from "./lib/state/agents.svelte";

  const q = new URLSearchParams(location.search);
  let prompt = $state(q.get("prompt") ?? "");
  let switcher = $state(false);
  let usage = $state(false);

  onMount(async () => {
    document.title = "Oltremare Studio";
    await settings.load().catch(() => {});
    await Promise.all([studio.loadProjects(), studio.loadClaudex(), dev.init(), agents.load().catch(() => {})]);
    const p = q.get("project");
    if (p) studio.open(p);
    on("studio-open", (e) => {
      if (e.project && e.project !== studio.project?.path) studio.open(e.project);
      if (e.prompt) prompt = e.prompt;
    });
  });

  function key(e: KeyboardEvent) {
    const mod = e.metaKey || e.ctrlKey;
    if (mod && e.key.toLowerCase() === "p") {
      e.preventDefault();
      switcher = !switcher;
    }
  }
</script>

<svelte:window onkeydown={key} />

<div class="studio" class:mac={ui.isMac}>
  <header class="bar" data-tauri-drag-region>
    <span class="brand" data-tauri-drag-region><span class="dot"></span><em>Oltremare</em> Studio</span>
    {#if studio.project}
      <button class="crumb" onclick={() => studio.close()} title="Tutti i progetti"><Icon name="left" size={13} /> Progetti</button>
      <span class="sep">/</span>
      <div class="proj">
        <button class="pname" onclick={() => (switcher = !switcher)} title="Cambia progetto (⌘P)">{studio.project.name} <Icon name="down" size={11} /></button>
        {#if switcher}
          <div class="menu" role="menu">
            {#each studio.projects.filter((p) => studio.recent.includes(p.path) || studio.pinned.includes(p.path)).slice(0, 10) as p (p.path)}
              <button role="menuitem" onclick={() => { switcher = false; studio.open(p); }}>{p.name}<small>{p.group || p.root}</small></button>
            {/each}
            <button role="menuitem" class="all" onclick={() => { switcher = false; studio.close(); }}>Tutti i progetti…</button>
          </div>
        {/if}
      </div>
      {#if dev.git?.repo}<span class="branch" data-tauri-drag-region><Icon name="layers" size={12} /> {dev.git.branch}</span>{/if}
    {/if}
    <span class="sp" data-tauri-drag-region></span>
    {#if studio.project}<RunBar />{/if}
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

  {#if studio.project}
    {#key studio.project.path}
      <Workspace {prompt} />
    {/key}
  {:else}
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
  .crumb {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-left: 10px;
    padding: 4px 8px;
    border-radius: 6px;
    font-size: var(--t-sm);
    color: var(--ink-3);
  }
  .crumb:hover,
  .pname:hover {
    background: var(--paper);
  }
  .sep {
    color: var(--ink-3);
  }
  .proj {
    position: relative;
  }
  .pname {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 4px 8px;
    border-radius: 6px;
    font-weight: 650;
  }
  .menu {
    position: absolute;
    top: 32px;
    left: 0;
    z-index: 30;
    min-width: 280px;
    padding: 6px;
    display: grid;
    border-radius: var(--r-md);
    background: var(--paper);
    box-shadow: var(--shadow-sheet);
  }
  .menu button {
    display: flex;
    justify-content: space-between;
    gap: 10px;
    padding: 7px 9px;
    border-radius: 6px;
    font-size: var(--t-sm);
    text-align: left;
  }
  .menu button:hover {
    background: var(--paper-2);
  }
  .menu small {
    color: var(--ink-3);
  }
  .menu .all {
    color: var(--accent);
  }
  .branch {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: var(--t-xs);
    color: var(--accent);
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
