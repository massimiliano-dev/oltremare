<script lang="ts">
  import { onMount } from "svelte";
  import Icon from "./Icon.svelte";
  import { fadeOut } from "./motion";
  import { ui } from "./state/ui.svelte";
  import { agents } from "./state/agents.svelte";
  import { runs } from "./state/runs.svelte";
  import { settings } from "./state/settings.svelte";
  import { theme } from "./state/theme.svelte";
  import { api } from "./api";

  interface Command {
    label: string;
    hint?: string;
    icon: string;
    keys?: string;
    run: () => void;
  }

  let q = $state("");
  let active = $state(0);
  let input: HTMLInputElement | undefined = $state();

  const commands = $derived.by<Command[]>(() => {
    const a = agents.selected;
    const list: Command[] = [
      { label: "Nuova conversazione", icon: "pen", keys: "⌘N", run: () => runs.newConversation() },
      { label: "Scrivi un messaggio", icon: "chat", keys: "⌘L", run: () => setTimeout(() => document.getElementById("composer")?.focus()) },
      { label: "Nuovo agente", icon: "plus", keys: "⌘⇧A", run: () => (ui.editing = "new") },
    ];
    if (a) {
      list.push({ label: `Modifica ${a.name}`, icon: "settings", keys: "⌘E", run: () => (ui.editing = $state.snapshot(a)) });
      if (!a.system) list.push({ label: `Duplica ${a.name}`, icon: "copy", run: () => agents.duplicate(a) });
    }
    if (runs.active) list.push({ label: "Ferma l'esecuzione", icon: "stop", keys: "⌘.", run: () => runs.cancel(runs.active!.id) });
    list.push({ label: "Ferma tutti gli agenti", hint: "interruttore di emergenza", icon: "shield", keys: ui.isMac ? "⌘⇧⎋" : "Ctrl+Alt+Esc", run: () => api.stopEverything() });
    for (const x of agents.list) {
      if (x.id !== a?.id) list.push({ label: `Passa a ${x.name}`, hint: x.description, icon: x.icon, run: () => agents.select(x.id) });
    }
    for (const c of runs.conversations.slice(0, 20)) {
      list.push({ label: c.title, hint: "conversazione", icon: "chat", run: () => runs.openConversation(c.id) });
    }
    list.push(
      { label: theme.resolved === "dark" ? "Tema chiaro" : "Tema scuro", icon: theme.resolved === "dark" ? "sun" : "moon", run: () => settings.save({ theme: theme.resolved === "dark" ? "light" : "dark" }) },
      { label: avatar() === "orb" ? "Volto: Personaggio" : "Volto: Orb", icon: avatar() === "orb" ? "face" : "orb", run: () => settings.save({ avatarFace: avatar() === "orb" ? "character" : "orb" }) },
      { label: "Impostazioni", icon: "settings", keys: "⌘,", run: () => ui.openSettings() },
      { label: "Collega Claude o GPT", hint: "chiavi API", icon: "key", run: () => ui.openSettings("providers") },
    );
    return list;
  });

  function avatar() {
    return settings.s?.avatarFace ?? "character";
  }

  // Ricerca fuzzy leggera: tutte le lettere in ordine, con bonus per le iniziali di parola
  function score(label: string, query: string): number {
    if (!query) return 1;
    const l = label.toLowerCase();
    const qq = query.toLowerCase();
    if (l.includes(qq)) return 100 - l.indexOf(qq);
    let i = 0;
    let s = 0;
    for (const ch of qq) {
      const j = l.indexOf(ch, i);
      if (j < 0) return 0;
      s += j === 0 || l[j - 1] === " " ? 3 : 1;
      i = j + 1;
    }
    return s;
  }

  const results = $derived(
    commands
      .map((c) => ({ c, s: score(c.label, q.trim()) }))
      .filter((x) => x.s > 0)
      .sort((a, b) => b.s - a.s)
      .slice(0, 12)
      .map((x) => x.c),
  );

  $effect(() => {
    void q;
    active = 0;
  });

  function exec(c: Command | undefined) {
    if (!c) return;
    ui.showPalette = false;
    c.run();
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      active = (active + 1) % Math.max(1, results.length);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      active = (active - 1 + results.length) % Math.max(1, results.length);
    } else if (e.key === "Enter") {
      e.preventDefault();
      exec(results[active]);
    }
  }

  onMount(() => input?.focus());
</script>

<div class="scrim" role="presentation" onclick={() => (ui.showPalette = false)} out:fadeOut={{ duration: 120 }}></div>
<div class="palette" role="dialog" aria-label="Comandi" out:fadeOut={{ duration: 120 }}>
  <div class="search">
    <Icon name="command" size={16} />
    <input bind:this={input} bind:value={q} onkeydown={onKey} placeholder="Cosa vuoi fare?" />
  </div>
  <ul>
    {#each results as c, i (c.label + i)}
      <li>
        <button class:active={i === active} onmouseenter={() => (active = i)} onclick={() => exec(c)}>
          <Icon name={c.icon} size={16} />
          <span class="label">{c.label}</span>
          {#if c.hint}<small>{c.hint}</small>{/if}
          {#if c.keys}<kbd>{c.keys}</kbd>{/if}
        </button>
      </li>
    {:else}
      <li class="none">Nessun comando</li>
    {/each}
  </ul>
</div>

<style>
  .scrim {
    position: absolute;
    inset: 0;
    z-index: 80;
    background: var(--scrim);
  }
  .palette {
    position: absolute;
    z-index: 81;
    top: 14vh;
    left: 50%;
    width: min(580px, calc(100vw - 32px));
    transform: translateX(-50%);
    border-radius: 16px;
    background: var(--paper);
    box-shadow: var(--shadow-sheet), 0 0 0 1px var(--rule);
    overflow: hidden;
    animation: pop 0.2s var(--ease-out);
  }
  @keyframes pop {
    from {
      opacity: 0;
      transform: translateX(-50%) translateY(-8px) scale(0.98);
    }
  }
  .search {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 18px;
    height: 56px;
    color: var(--accent);
    border-bottom: 1px solid var(--rule-soft);
  }
  .search input {
    flex: 1;
    border: 0;
    outline: 0;
    background: none;
    font-size: 1.05rem;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 6px;
    max-height: 420px;
    overflow-y: auto;
  }
  li button {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    height: 40px;
    padding: 0 12px;
    border-radius: var(--r-sm);
    color: var(--ink-2);
    font-size: var(--t-md);
    text-align: left;
  }
  li button.active {
    background: var(--accent-mist);
    color: var(--accent);
  }
  .label {
    flex: 1;
    color: var(--ink);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  small {
    font-size: var(--t-xs);
    color: var(--ink-3);
    max-width: 40%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .none {
    padding: 16px;
    color: var(--ink-3);
    font-size: var(--t-sm);
  }
</style>
