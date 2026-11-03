<script lang="ts">
  // Cerca nel progetto (⌘⇧F): righe trovate raggruppate per file; un clic apre il file alla riga.
  import { getContext, onMount, tick } from "svelte";
  import Icon from "../Icon.svelte";
  import FileIcon from "../dev/FileIcon.svelte";
  import { api, errorText } from "../api";
  import type { Workspace as WS } from "../state/workspace.svelte";

  const ws = getContext<WS>("ws");
  // Testo e modalità vivono nella scheda: «vai alla definizione» può aprire la ricerca già compilata
  let q = $derived(ws.searchQuery);
  let regex = $derived(ws.searchRegex);
  let cs = $state(false);
  let results = $state<{ file: string; path: string; line: number; col: number; text: string }[]>([]);
  let busy = $state(false);
  let error = $state("");
  let input: HTMLInputElement | undefined = $state();
  let timer: ReturnType<typeof setTimeout> | undefined;
  let closed = $state<Record<string, boolean>>({});

  const groups = $derived.by(() => {
    const m = new Map<string, typeof results>();
    for (const r of results) m.set(r.file, [...(m.get(r.file) ?? []), r]);
    return [...m.entries()];
  });

  async function run() {
    const query = q;
    if (!query.trim()) {
      results = [];
      error = "";
      return;
    }
    busy = true;
    try {
      const r = await api.ideSearch(ws.path, query, regex, cs);
      if (query === q) {
        results = r;
        error = "";
      }
    } catch (e) {
      error = errorText(e);
      results = [];
    } finally {
      busy = false;
    }
  }

  $effect(() => {
    void q;
    void regex;
    void cs;
    clearTimeout(timer);
    timer = setTimeout(run, 250);
  });

  /** La riga con la parte trovata evidenziata */
  function parts(text: string) {
    try {
      const re = new RegExp(regex ? q : q.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), cs ? "" : "i");
      const m = text.match(re);
      if (!m || m.index === undefined) return [text, "", ""];
      const start = Math.max(0, m.index - 40);
      return [(start ? "…" : "") + text.slice(start, m.index).trimStart(), m[0], text.slice(m.index + m[0].length)];
    } catch {
      return [text, "", ""];
    }
  }

  onMount(async () => {
    await tick();
    input?.focus();
  });
</script>

<div class="search">
  <div class="field">
    <input bind:this={input} value={q} oninput={(e) => (ws.searchQuery = (e.target as HTMLInputElement).value)} placeholder="Cerca nel progetto" aria-label="Cerca nel progetto" onkeydown={(e) => e.key === "Enter" && run()} />
    <button class="opt" class:on={cs} onclick={() => (cs = !cs)} title="Maiuscole e minuscole" aria-pressed={cs}>Aa</button>
    <button class="opt" class:on={regex} onclick={() => (ws.searchRegex = !ws.searchRegex)} title="Espressione regolare" aria-pressed={regex}>.*</button>
  </div>
  {#if error}<p class="msg err">{error}</p>
  {:else if busy && !results.length}<p class="msg">Cerco…</p>
  {:else if q.trim() && !results.length}<p class="msg">Nessun risultato</p>
  {:else if results.length}<p class="msg">{results.length >= 2000 ? "Oltre 2000" : results.length} risultati in {groups.length} file</p>{/if}
  <div class="res">
    {#each groups as [file, rows] (file)}
      {@const nome = file.split("/").pop() ?? file}
      <button class="fh" onclick={() => (closed[file] = !closed[file])} aria-expanded={!closed[file]}>
        <span class="chev" class:open={!closed[file]}><Icon name="chevron" size={10} /></span>
        <FileIcon name={nome} />
        <span class="fn">{nome}</span>
        <span class="fd">{file.includes("/") ? file.slice(0, file.lastIndexOf("/")) : ""}</span>
        <span class="cnt">{rows.length}</span>
      </button>
      {#if !closed[file]}
        {#each rows as r (r.line + ":" + r.col)}
          {@const [a, b, c] = parts(r.text)}
          <button class="row" onclick={() => ws.openFile(r.path, r.line, r.col)} title="{file}:{r.line}">
            <span class="ln">{r.line}</span>
            <span class="tx">{a}<mark>{b}</mark>{c}</span>
          </button>
        {/each}
      {/if}
    {/each}
  </div>
</div>

<style>
  .search {
    display: grid;
    gap: 6px;
    padding: 8px 4px;
  }
  .field {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 0 4px 0 10px;
    border-radius: 9px;
    background: var(--paper);
    box-shadow: inset 0 0 0 1px var(--rule);
  }
  .field:focus-within {
    box-shadow: inset 0 0 0 1.5px var(--accent);
  }
  .field input {
    flex: 1;
    min-width: 0;
    height: 30px;
    border: 0;
    background: none;
    outline: none;
    font-size: var(--t-sm);
    color: var(--ink);
  }
  .opt {
    width: 24px;
    height: 22px;
    border-radius: 5px;
    font-size: 0.7rem;
    font-weight: 700;
    color: var(--ink-3);
    font-family: ui-monospace, Menlo, monospace;
  }
  .opt.on {
    color: var(--accent);
    background: color-mix(in oklch, var(--accent) 14%, transparent);
  }
  .msg {
    margin: 0 6px;
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
  .msg.err {
    color: var(--danger);
  }
  .res {
    display: grid;
  }
  .fh {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 26px;
    padding: 0 6px;
    border-radius: 7px;
    font-size: var(--t-sm);
    text-align: left;
  }
  .fh:hover,
  .row:hover {
    background: color-mix(in oklch, var(--accent) 7%, transparent);
  }
  .chev {
    display: grid;
    color: var(--ink-3);
    transition: transform 0.15s;
  }
  .chev.open {
    transform: rotate(90deg);
  }
  .fn {
    flex: none;
    font-weight: 600;
  }
  .fd {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
  .cnt {
    flex: none;
    min-width: 18px;
    padding: 0 5px;
    border-radius: 999px;
    font-size: 0.64rem;
    font-weight: 700;
    color: var(--ink-2);
    background: var(--paper-3);
    text-align: center;
  }
  .row {
    display: flex;
    gap: 8px;
    align-items: baseline;
    padding: 3px 6px 3px 28px;
    border-radius: 6px;
    text-align: left;
    font-size: 0.74rem;
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
    min-width: 0;
  }
  .ln {
    flex: none;
    min-width: 28px;
    color: var(--ink-3);
    text-align: right;
  }
  .tx {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--ink-2);
  }
  mark {
    color: var(--ink);
    background: color-mix(in oklch, var(--warn) 35%, transparent);
    border-radius: 3px;
  }
</style>
