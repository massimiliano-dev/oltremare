<script lang="ts">
  import Icon from "./Icon.svelte";
  import { api } from "./api";
  import { ui } from "./state/ui.svelte";
  import { runs } from "./state/runs.svelte";
  import { agents } from "./state/agents.svelte";
  import { avatar } from "./state/avatar.svelte";
  import { local } from "./state/local.svelte";
</script>

<header class="bar" class:mac={ui.isMac} data-tauri-drag-region>
  <div class="brand" data-tauri-drag-region>
    <span class="dot s-{avatar.shown}" title={avatar.label}></span>
    <span class="word" data-tauri-drag-region>Oltremare</span>
  </div>
  <div class="crumb" data-tauri-drag-region>
    {#if agents.selected}
      <span data-tauri-drag-region>{agents.selected.name}</span>
      <Icon name="chevron" size={12} />
      <span class="conv" data-tauri-drag-region>{runs.conversation?.title ?? "Nuova conversazione"}</span>
    {/if}
  </div>
  <div class="actions">
    {#if local.active}
      <button class="localchip st-{local.active.state}" onclick={() => ui.openSettings("local")} title="AI locale">
        <span class="led"></span> {local.label(local.active.model)}
      </button>
    {/if}
    {#if runs.active}
      <button class="running" onclick={() => runs.cancel(runs.active!.id)} title="Ferma (⌘.)">
        <span class="spin"></span> Al lavoro · Ferma
      </button>
    {/if}
    <button class="icon-btn" class:on={ui.view === "memory"} onclick={() => (ui.view = ui.view === "memory" ? "chat" : "memory")} title="Memoria: le note degli agenti come grafo (⌘⇧M)" aria-label="Memoria"><Icon name="book" size={17} /></button>
    <button class="icon-btn" onclick={() => api.studioOpen()} title="Oltremare Studio: la modalità sviluppo (⌘⇧D)" aria-label="Studio"><Icon name="terminal" size={17} /></button>
    <button class="icon-btn" onclick={() => (ui.showPalette = true)} title="Comandi (⌘K)" aria-label="Comandi"><Icon name="command" size={17} /></button>
    <button class="icon-btn" onclick={() => ui.openSettings()} title="Impostazioni (⌘,)" aria-label="Impostazioni"><Icon name="settings" size={17} /></button>
  </div>
</header>

<style>
  .bar {
    display: grid;
    grid-template-columns: 248px 1fr auto;
    align-items: center;
    height: 46px;
    padding: 0 10px 0 16px;
    border-bottom: 1px solid var(--rule-soft);
    background: var(--paper-2);
  }
  .bar.mac {
    padding-left: 86px;
    grid-template-columns: 178px 1fr auto;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 9px;
  }
  .word {
    font-family: var(--font-serif);
    font-style: italic;
    font-size: 1.32rem;
    color: var(--accent);
    letter-spacing: 0.01em;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-mist);
    transition: background 0.3s;
  }
  .dot.s-thinking,
  .dot.s-working,
  .dot.s-speaking {
    animation: beat 1s ease-in-out infinite;
  }
  .dot.s-asking,
  .dot.s-alert {
    background: var(--warn);
  }
  .dot.s-error {
    background: var(--danger);
  }
  .dot.s-sleep {
    background: var(--ink-3);
  }
  @keyframes beat {
    50% {
      box-shadow: 0 0 0 6px var(--accent-mist);
    }
  }
  .crumb {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    font-size: var(--t-sm);
    color: var(--ink-3);
  }
  .crumb .conv {
    color: var(--ink);
    font-weight: 550;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .localchip {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 26px;
    padding: 0 10px;
    margin-right: 4px;
    border-radius: 999px;
    font-size: var(--t-xs);
    font-weight: 600;
    color: var(--ink-2);
    box-shadow: inset 0 0 0 1px var(--rule);
  }
  .localchip .led {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--warn);
  }
  .localchip.st-ready .led {
    background: var(--ok);
  }
  .localchip.st-error .led {
    background: var(--danger);
  }
  .running {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 28px;
    padding: 0 12px;
    margin-right: 6px;
    border-radius: 999px;
    font-size: var(--t-xs);
    font-weight: 650;
    color: var(--accent);
    background: var(--accent-mist);
    box-shadow: inset 0 0 0 1px var(--accent-wash);
  }
  .running:hover {
    color: var(--danger);
    background: var(--danger-wash);
  }
  .spin {
    width: 11px;
    height: 11px;
    border-radius: 50%;
    border: 2px solid currentColor;
    border-right-color: transparent;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .dot,
    .spin {
      animation: none !important;
    }
  }
</style>
