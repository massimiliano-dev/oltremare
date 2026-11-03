<script lang="ts">
  // Apertura rapida (⌘P): cerca un file per nome tra tutti quelli del progetto.
  import { getContext, onMount, tick } from "svelte";
  import FileIcon from "../dev/FileIcon.svelte";
  import { api } from "../api";
  import type { Workspace as WS } from "../state/workspace.svelte";

  const ws = getContext<WS>("ws");
  let files = $state<string[]>([]);
  let q = $state("");
  let sel = $state(0);
  let input: HTMLInputElement | undefined = $state();

  /** Punteggio: le lettere in ordine; meglio se consecutive, nel nome del file, all'inizio di una parola */
  function score(path: string, query: string) {
    if (!query) return 1;
    const p = path.toLowerCase();
    const nome = p.slice(p.lastIndexOf("/") + 1);
    let i = 0;
    let s = 0;
    let prev = -2;
    for (const c of query.toLowerCase()) {
      if (c === " ") continue;
      const j = p.indexOf(c, i);
      if (j < 0) return 0;
      s += j === prev + 1 ? 3 : 1;
      if (j >= p.length - nome.length) s += 2;
      if (j === 0 || "/-_.".includes(p[j - 1])) s += 2;
      prev = j;
      i = j + 1;
    }
    return s - nome.length / 10 - p.length / 200;
  }

  const results = $derived(
    files
      .map((f) => ({ f, s: score(f, q) }))
      .filter((x) => x.s > 0)
      .sort((a, b) => b.s - a.s)
      .slice(0, 60)
      .map((x) => x.f),
  );

  function open(rel: string) {
    ws.quickOpen = false;
    ws.openFile(`${ws.path}/${rel}`);
  }

  function key(e: KeyboardEvent) {
    if (e.key === "Escape") ws.quickOpen = false;
    else if (e.key === "ArrowDown") {
      e.preventDefault();
      sel = Math.min(sel + 1, results.length - 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      sel = Math.max(sel - 1, 0);
    } else if (e.key === "Enter" && results[sel]) open(results[sel]);
  }

  onMount(async () => {
    await tick();
    input?.focus();
    files = await api.ideFiles(ws.path).catch(() => []);
  });

  $effect(() => {
    void q;
    sel = 0;
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
<div class="scrim" onclick={() => (ws.quickOpen = false)}>
  <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
  <div class="box" onclick={(e) => e.stopPropagation()} role="dialog" tabindex="-1" aria-label="Apri un file">
    <input bind:this={input} bind:value={q} onkeydown={key} placeholder={files.length ? `Cerca tra ${files.length} file…` : "Leggo i file del progetto…"} aria-label="Nome del file" />
    <ul role="listbox">
      {#each results as f, i (f)}
        {@const nome = f.split("/").pop() ?? f}
        <li role="option" aria-selected={i === sel}>
          <button class:on={i === sel} onmouseenter={() => (sel = i)} onclick={() => open(f)}>
            <FileIcon name={nome} />
            <span class="n">{nome}</span>
            <span class="d">{f.includes("/") ? f.slice(0, f.lastIndexOf("/")) : ""}</span>
          </button>
        </li>
      {/each}
      {#if files.length && !results.length}<li class="none">Nessun file con questo nome</li>{/if}
    </ul>
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 70;
    display: flex;
    justify-content: center;
    padding-top: 12vh;
    background: oklch(0.2 0.02 var(--tint-h) / 0.25);
  }
  .box {
    width: min(620px, 92vw);
    max-height: 60vh;
    display: flex;
    flex-direction: column;
    border-radius: 14px;
    overflow: hidden;
    background: var(--paper);
    box-shadow:
      0 0 0 1px var(--rule),
      0 24px 60px -18px oklch(0.2 0.06 var(--tint-h) / 0.45);
  }
  input {
    border: 0;
    border-bottom: 1px solid var(--rule-soft);
    padding: 14px 16px;
    font-size: var(--t-md, 1rem);
    background: var(--paper);
    color: var(--ink);
    outline: none;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 6px;
    overflow-y: auto;
  }
  li button {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    border-radius: 8px;
    text-align: left;
    font-size: var(--t-sm);
  }
  li button.on {
    background: color-mix(in oklch, var(--accent) 12%, transparent);
  }
  .n {
    flex: none;
    color: var(--ink);
    font-weight: 550;
  }
  .d {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--ink-3);
    font-size: var(--t-xs);
  }
  .none {
    padding: 12px;
    color: var(--ink-3);
    font-size: var(--t-sm);
  }
</style>
