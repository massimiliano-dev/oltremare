<script lang="ts">
  import { onMount } from "svelte";
  import Titlebar from "./lib/Titlebar.svelte";
  import AgentsPanel from "./lib/panels/AgentsPanel.svelte";
  import ChatPanel from "./lib/panels/ChatPanel.svelte";
  import AvatarPanel from "./lib/panels/AvatarPanel.svelte";
  import TimelinePanel from "./lib/panels/TimelinePanel.svelte";
  import AgentEditor from "./lib/agents/AgentEditor.svelte";
  import Settings from "./lib/settings/Settings.svelte";
  import Palette from "./lib/Palette.svelte";
  import Toasts from "./lib/Toasts.svelte";
  import Lightbox from "./lib/run/Lightbox.svelte";
  import { ui } from "./lib/state/ui.svelte";
  import { settings } from "./lib/state/settings.svelte";
  import { agents } from "./lib/state/agents.svelte";
  import { runs } from "./lib/state/runs.svelte";
  import { avatar } from "./lib/state/avatar.svelte";
  import { approvals } from "./lib/state/approvals.svelte";
  import { local } from "./lib/state/local.svelte";
  import { voice } from "./lib/state/voice.svelte";
  import { accounts } from "./lib/state/accounts.svelte";
  import { notifications } from "./lib/state/notifications.svelte";
  import { mcp } from "./lib/state/mcp.svelte";
  import { layout } from "./lib/state/layout.svelte";
  import Gutter from "./lib/Gutter.svelte";
  import { on } from "./lib/events";
  import { errorText } from "./lib/api";

  let ready = $state(false);

  onMount(async () => {
    try {
      runs.listen();
      await Promise.all([settings.load(), avatar.listen(), approvals.listen(), local.init(), voice.init(), accounts.init(), notifications.init(), mcp.init()]);
      // Esecuzioni partite fuori dalla finestra (palette, briefing, trigger): si aprono qui
      on("open-run", (r) => runs.openRun(r.agentId, r.conversationId));
      on("open-settings", (tab) => ui.openSettings(tab as never));
      on("trigger-fired", (t) => {
        const a = agents.byId(t.agentId);
        if (a) ui.toast(`«${a.name}» è partito da solo (${t.kind === "cron" ? "orario" : t.kind === "folder" ? "cartella" : t.kind === "mail" ? "email" : t.kind === "hotkey" ? "scorciatoia" : "notifica"})`, "info");
      });
      await Promise.all([agents.init(), settings.checkClaudeCode()]);
      await agents.adoptSubscription();
      if (agents.selected) await runs.selectAgent(agents.selected.id);
    } catch (e) {
      ui.toast(errorText(e), "error");
    } finally {
      ready = true;
    }
  });

  function typing(e: KeyboardEvent) {
    const t = e.target as HTMLElement;
    return t.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(t.tagName);
  }

  const overlay = $derived(ui.showPalette || ui.showSettings || !!ui.editing);

  function onKey(e: KeyboardEvent) {
    const mod = e.metaKey || e.ctrlKey;
    const k = e.key.toLowerCase();
    // Approvazioni prima di tutto, se non si sta scrivendo
    const req = approvals.current;
    if (req && !overlay && !typing(e)) {
      if (e.key === "Enter" && e.shiftKey && req.allowSession) {
        e.preventDefault();
        approvals.answer(req, "allowSession");
        return;
      }
      if (e.key === "Enter" && (req.risk !== "danger" || mod)) {
        e.preventDefault();
        approvals.answer(req, "allow");
        return;
      }
      if (e.key === "Escape") {
        e.preventDefault();
        approvals.answer(req, "deny");
        return;
      }
    }
    if (mod && k === "k") {
      e.preventDefault();
      ui.showPalette = !ui.showPalette;
      return;
    }
    if (mod && k === ",") {
      e.preventDefault();
      ui.openSettings();
      return;
    }
    if (mod && k === ".") {
      e.preventDefault();
      if (runs.active) runs.cancel(runs.active.id);
      return;
    }
    if (overlay) {
      if (e.key === "Escape" && !ui.editing) {
        if (ui.showPalette) ui.showPalette = false;
        else if (ui.showSettings) ui.showSettings = false;
      }
      return;
    }
    if (mod && e.shiftKey && k === "a") {
      e.preventDefault();
      ui.editing = "new";
      return;
    }
    if (mod && k === "n") {
      e.preventDefault();
      runs.newConversation();
      document.getElementById("composer")?.focus();
      return;
    }
    if (mod && k === "e" && agents.selected) {
      e.preventDefault();
      ui.editing = $state.snapshot(agents.selected);
      return;
    }
    if (mod && k === "b") {
      e.preventDefault();
      layout.toggle("agents");
      return;
    }
    if (mod && k === "j") {
      e.preventDefault();
      layout.toggle("side");
      return;
    }
    if (mod && k === "l") {
      e.preventDefault();
      document.getElementById("composer")?.focus();
      return;
    }
    // ⌘1…9: passa all'agente
    if (mod && /^[1-9]$/.test(e.key)) {
      const a = agents.list[Number(e.key) - 1];
      if (a) {
        e.preventDefault();
        agents.select(a.id);
      }
      return;
    }
    // Basta iniziare a scrivere per andare nel composer
    if (!typing(e) && !mod && !e.altKey && e.key.length === 1) document.getElementById("composer")?.focus();
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="shell" class:mac={ui.isMac}>
  <Titlebar />
  <main class="grid" class:ready style="grid-template-columns: {layout.cols.map((c, i) => (i ? 'auto ' : '') + (c.kind === 'chat' ? 'minmax(0, 1fr)' : `${layout.width(c.kind)}px`)).join(' ')}">
    {#each layout.cols as c, i (c.kind)}
      {#if i > 0}<Gutter left={layout.cols[i - 1].kind} right={c.kind} />{/if}
      {#if c.kind === "agents"}
        <AgentsPanel />
      {:else if c.kind === "chat"}
        <ChatPanel />
      {:else}
        <aside class="side">
          <AvatarPanel />
          <TimelinePanel />
        </aside>
      {/if}
    {/each}
  </main>
  {#if ui.editing}
    {#key ui.editing}
      <AgentEditor agent={ui.editing} />
    {/key}
  {/if}
  {#if ui.showSettings && settings.s}
    <Settings />
  {/if}
  {#if ui.showPalette}
    <Palette />
  {/if}
  <Lightbox />
  <Toasts />
</div>

<style>
  .shell {
    position: relative;
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--paper);
  }
  .grid {
    flex: 1;
    min-height: 0;
    display: grid;
    opacity: 0;
    transition: opacity 0.25s;
  }
  .grid.ready {
    opacity: 1;
  }
  .side {
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--paper);
  }
</style>
