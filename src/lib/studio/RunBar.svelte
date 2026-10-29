<script lang="ts">
  // Avvio delle app in locale: il comando principale a un clic, gli altri in un menu, i processi accesi con stop e anteprima.
  import Icon from "../Icon.svelte";
  import { api, errorText } from "../api";
  import { studio } from "../state/studio.svelte";
  import { ui } from "../state/ui.svelte";
  import type { Workspace as WS } from "../state/workspace.svelte";

  let { ws }: { ws: WS } = $props();

  type Cmd = { label: string; command: string; cwd: string; rel: string; kind: string; priority: number };
  let cmds = $state<Cmd[]>([]);
  let menu = $state(false);
  let busy = $state<string | null>(null);
  const primary = $derived(cmds.find((c) => c.kind !== "setup" && c.kind !== "task") ?? cmds[0] ?? null);
  const running = $derived(ws.dev.processes.filter((p) => p.running));

  $effect(() => {
    const p = ws.project?.path;
    cmds = [];
    if (p) api.studioCommands(p).then((c) => (cmds = c)).catch(() => (cmds = []));
  });

  async function launch(c: Cmd) {
    if (!ws.project) return;
    menu = false;
    busy = c.command + c.cwd;
    try {
      await api.studioLaunch(ws.project.path, c.cwd, c.command, c.label);
      await ws.dev.refreshProcesses();
      const p = ws.dev.processes.find((x) => x.name === c.label);
      if (p) ws.dev.bottom = p.id;
      if (p?.urls.length) {
        ws.dev.previewUrl = p.urls[0];
        ws.dev.tab = "preview";
      }
    } catch (e) {
      ui.toast(errorText(e), "error");
      ws.dev.refreshProcesses();
    } finally {
      busy = null;
    }
  }

  async function stop(id: string) {
    await api.studioStop(id).catch((e) => ui.toast(errorText(e), "error"));
    ws.dev.refreshProcesses();
  }

  const ICON: Record<string, string> = { tauri: "cpu", web: "globe", docker: "layers", rust: "terminal", python: "terminal", flutter: "image", dotnet: "terminal", go: "terminal", task: "check", setup: "download" };
</script>

<div class="run">
  {#each running as p (p.id)}
    <span class="proc">
      <span class="led"></span>
      <button class="pn" onclick={() => { ws.dev.bottom = p.id; }} title={p.command}>{p.name}</button>
      {#if p.urls[0]}<button class="pa" onclick={() => { ws.dev.previewUrl = p.urls[0]; ws.dev.tab = "preview"; }} title="Apri l'anteprima">{p.urls[0].replace(/^https?:\/\//, "").replace(/\/$/, "")}</button>{/if}
      <button class="px" onclick={() => stop(p.id)} title="Ferma" aria-label="Ferma {p.name}"><Icon name="stop" size={10} /></button>
    </span>
  {/each}
  {#if primary}
    <div class="split">
      <button class="go" onclick={() => launch(primary)} disabled={!!busy} title={`${primary.command}${primary.rel ? ` (in ${primary.rel})` : ""}`}>
        {#if busy === primary.command + primary.cwd}<span class="spin"></span>{:else}<Icon name="play" size={12} />{/if}
        {primary.label}
      </button>
      <button class="more" onclick={() => (menu = !menu)} aria-label="Altri comandi" aria-expanded={menu}><Icon name="down" size={12} /></button>
      {#if menu}
        <div class="menu" role="menu">
          {#each cmds as c (c.command + c.cwd)}
            <button role="menuitem" onclick={() => launch(c)} disabled={!!busy}>
              <span class="mi"><Icon name={ICON[c.kind] ?? "play"} size={13} /></span>
              <span class="mt"><strong>{c.label}</strong><small>{c.command}</small></span>
            </button>
          {/each}
        </div>
      {/if}
    </div>
  {:else if ws.project}
    <span class="none" title="Nessun comando di avvio riconosciuto">Nessun avvio trovato</span>
  {/if}
</div>

<style>
  .run {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .proc {
    display: flex;
    align-items: center;
    gap: 4px;
    height: 26px;
    padding: 0 4px 0 8px;
    border-radius: 999px;
    font-size: var(--t-xs);
    background: oklch(0.78 0.13 150 / 0.2);
    color: oklch(0.42 0.12 150);
  }
  :global([data-theme="dark"]) .proc {
    background: oklch(0.6 0.12 150 / 0.22);
    color: oklch(0.85 0.12 150);
  }
  .led {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: oklch(0.65 0.17 150);
    box-shadow: 0 0 0 3px oklch(0.65 0.17 150 / 0.2);
    animation: on 1.6s ease-in-out infinite;
  }
  @keyframes on {
    50% {
      box-shadow: 0 0 0 5px oklch(0.65 0.17 150 / 0.08);
    }
  }
  .pn {
    font-weight: 650;
  }
  .pa {
    font-family: ui-monospace, Menlo, monospace;
    font-size: 0.68rem;
    text-decoration: underline;
    text-underline-offset: 2px;
  }
  .px {
    display: grid;
    place-items: center;
    width: 18px;
    height: 18px;
    border-radius: 50%;
  }
  .px:hover {
    background: oklch(0.65 0.17 25 / 0.2);
    color: oklch(0.55 0.17 25);
  }
  .split {
    position: relative;
    display: flex;
  }
  .go {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 12px;
    border-radius: 999px 0 0 999px;
    font-size: var(--t-xs);
    font-weight: 650;
    color: white;
    background: linear-gradient(135deg, oklch(0.6 0.16 150), oklch(0.52 0.14 165));
    box-shadow: 0 6px 14px -8px oklch(0.45 0.14 155 / 0.8);
  }
  .go:disabled {
    opacity: 0.7;
  }
  .more {
    display: grid;
    place-items: center;
    width: 26px;
    height: 28px;
    border-radius: 0 999px 999px 0;
    color: white;
    background: oklch(0.5 0.13 160);
    border-left: 1px solid oklch(1 0 0 / 0.25);
  }
  .menu {
    position: absolute;
    top: 34px;
    right: 0;
    z-index: 40;
    min-width: 300px;
    max-height: 60vh;
    overflow-y: auto;
    padding: 6px;
    display: grid;
    border-radius: var(--r-md);
    background: var(--paper);
    box-shadow: var(--shadow-sheet);
  }
  .menu button {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 8px;
    border-radius: 8px;
    text-align: left;
  }
  .menu button:hover {
    background: color-mix(in oklch, var(--accent) 7%, transparent);
  }
  .mi {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 8px;
    color: oklch(0.5 0.13 155);
    background: oklch(0.78 0.13 150 / 0.2);
    flex: none;
  }
  .mt {
    display: grid;
    min-width: 0;
  }
  .mt strong {
    font-size: var(--t-sm);
    font-weight: 600;
  }
  .mt small {
    font-family: ui-monospace, Menlo, monospace;
    font-size: 0.68rem;
    color: var(--ink-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .none {
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
  .spin {
    width: 11px;
    height: 11px;
    border-radius: 50%;
    border: 2px solid oklch(1 0 0 / 0.35);
    border-top-color: white;
    animation: s 0.8s linear infinite;
  }
  @keyframes s {
    to {
      transform: rotate(360deg);
    }
  }
</style>
