<script lang="ts">
  // Task del gestionale delle issue collegato al progetto (server MCP): collegamento, elenco, «lavora su questa».
  import { onMount } from "svelte";
  import Icon from "../Icon.svelte";
  import { api, errorText } from "../api";
  import { studio } from "../state/studio.svelte";
  import { ui } from "../state/ui.svelte";
  import { markdown } from "../markdown";

  let info = $state<{ link: { server: string; project: string; name: string } | null; servers: string[]; canImport: boolean } | null>(null);
  let issues = $state<{ key: string; title: string; status: string; extra: string | null }[]>([]);
  let suggestions = $state<{ key: string; name: string }[]>([]);
  let manual = $state("");
  let server = $state("");
  let loading = $state(false);
  let open = $state(true);
  let detail = $state<{ key: string; text: string } | null>(null);
  const path = $derived(studio.project?.path ?? "");

  async function load() {
    if (!path) return;
    info = await api.studioLink(path).catch(() => null);
    server = info?.link?.server ?? info?.servers[0] ?? "";
    if (info?.link) {
      loading = true;
      try {
        issues = (await api.studioIssues(path)).issues;
      } catch (e) {
        issues = [];
        ui.toast(errorText(e), "error");
      } finally {
        loading = false;
      }
    } else if (server) {
      suggestions = await api.studioLinkSuggest(path, server).catch(() => []);
    }
  }

  onMount(load);

  async function connect() {
    try {
      const added = await api.mcpImportClaude();
      await api.mcpReload();
      ui.toast(added.length ? `Collegato: ${added.join(", ")}` : "Già collegato");
      await load();
    } catch (e) {
      ui.toast(errorText(e), "error");
    }
  }

  async function link(key: string, name: string) {
    await api.studioLinkSet(path, server, key, name);
    manual = "";
    await load();
  }

  async function show(key: string) {
    detail = { key, text: "…" };
    detail = { key, text: await api.studioIssue(path, key).catch((e) => errorText(e)) };
  }

  function work(i: { key: string; title: string }) {
    studio.draft = `Lavora sulla issue ${i.key} («${i.title}»): leggila con il gestionale (anche documentazione e commenti), segna l'inizio del lavoro, implementa, verifica e fai il commit con la chiave della issue.`;
    detail = null;
  }

  const STATO: Record<string, string> = { "Da fare": "todo", "In corso": "doing", "In revisione": "review", Fatto: "done" };
</script>

<div class="issues">
  <button class="sec" onclick={() => (open = !open)}>
    <Icon name={open ? "down" : "chevron"} size={11} /> Task
    {#if info?.link}<span class="key">{info.link.project}</span>{/if}
    {#if issues.length}<span class="n">{issues.length}</span>{/if}
  </button>
  {#if open}
    {#if !info}
      <div class="skeleton" style="height:40px"></div>
    {:else if !info.servers.length}
      <p class="hint">Collega il gestionale delle issue per vedere task e documentazione del progetto.</p>
      {#if info.canImport}<button class="btn btn-line sm" onclick={connect}><Icon name="layers" size={13} /> Collega da Claude Code</button>{/if}
    {:else if !info.link}
      <p class="hint">A quale progetto di <b>{server}</b> corrisponde questa cartella?</p>
      {#each suggestions as s (s.key)}
        <button class="sug" onclick={() => link(s.key, s.name)}><span class="key">{s.key}</span>{s.name}</button>
      {/each}
      <div class="manual">
        <input class="input" bind:value={manual} placeholder="Chiave, es. KEYO" spellcheck="false" />
        <button class="btn btn-line sm" disabled={!manual.trim()} onclick={() => link(manual, manual)}>Collega</button>
      </div>
    {:else}
      <div class="linked">
        <span>{info.link.name}</span>
        <button class="link" onclick={() => link("", "")}>scollega</button>
        <button class="icon-btn" onclick={load} title="Aggiorna" aria-label="Aggiorna"><Icon name="refresh" size={12} /></button>
      </div>
      {#if loading}
        <div class="skeleton" style="height:60px"></div>
      {:else}
        {#each issues as i (i.key)}
          <div class="iss">
            <button class="it" onclick={() => show(i.key)} title={i.title}>
              <span class="st st-{STATO[i.status] ?? 'todo'}"></span>
              <span class="ik">{i.key}</span>
              <span class="tt">{i.title}</span>
            </button>
            <button class="go" onclick={() => work(i)} title="Lavora su questa issue" aria-label="Lavora su {i.key}"><Icon name="play" size={11} /></button>
          </div>
        {:else}
          <p class="hint">Nessuna issue aperta assegnata a te in {info.link.project}.</p>
        {/each}
      {/if}
    {/if}
  {/if}
</div>

{#if detail}
  <div class="scrim" role="presentation" onclick={() => (detail = null)}></div>
  <div class="modal" role="dialog" aria-label="Issue {detail.key}">
    <header><strong>{detail.key}</strong><button class="icon-btn" onclick={() => (detail = null)} aria-label="Chiudi"><Icon name="x" size={15} /></button></header>
    <div class="body md">{@html markdown(detail.text)}</div>
    <footer>
      {#if issues.find((x) => x.key === detail?.key)}
        <button class="btn btn-primary sm" onclick={() => { const i = issues.find((x) => x.key === detail?.key); if (i) work(i); }}><Icon name="play" size={12} /> Lavora su questa issue</button>
      {/if}
    </footer>
  </div>
{/if}

<style>
  .issues {
    display: grid;
    gap: 4px;
    padding-bottom: 8px;
    border-bottom: 1px solid var(--rule-soft);
    margin-bottom: 4px;
  }
  .sec {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 8px 8px 2px;
    font-size: var(--t-xs);
    font-weight: 700;
    color: var(--ink-2);
    text-align: left;
  }
  .sec :global(svg) {
    color: var(--accent);
  }
  .key {
    padding: 1px 6px;
    border-radius: 5px;
    font-size: 0.66rem;
    font-weight: 700;
    letter-spacing: 0.02em;
    color: var(--accent);
    background: var(--accent-mist);
  }
  .n {
    margin-left: auto;
    letter-spacing: 0;
  }
  .hint {
    margin: 2px 8px;
    font-size: var(--t-xs);
    color: var(--ink-3);
    line-height: 1.45;
  }
  .sm {
    height: 28px;
    margin: 2px 8px;
    padding: 0 10px;
    font-size: var(--t-xs);
    gap: 5px;
    justify-self: start;
  }
  .sug {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 4px;
    padding: 6px 8px;
    border-radius: 6px;
    font-size: var(--t-sm);
    text-align: left;
  }
  .sug:hover {
    background: var(--paper);
  }
  .manual {
    display: flex;
    gap: 6px;
    margin: 2px 8px;
  }
  .manual .input {
    height: 28px;
    font-size: var(--t-xs);
  }
  .manual .sm {
    margin: 0;
  }
  .linked {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0 8px;
    font-size: var(--t-xs);
    color: var(--ink-2);
  }
  .linked span {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .linked .link {
    font-size: 0.7rem;
  }
  .iss {
    display: flex;
    align-items: center;
    margin: 0 2px;
    border-radius: 8px;
    transition: background-color 0.12s;
  }
  .iss:hover {
    background: color-mix(in oklch, var(--accent) 7%, transparent);
  }
  .it {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 6px;
    font-size: var(--t-sm);
    text-align: left;
  }
  .ik {
    flex: none;
    font-size: 0.7rem;
    font-weight: 700;
    color: var(--ink-3);
  }
  .tt {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .st {
    flex: none;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    box-shadow: inset 0 0 0 1.5px var(--ink-3);
  }
  .st-doing {
    background: var(--accent);
    box-shadow: none;
  }
  .st-review {
    background: oklch(0.72 0.14 80);
    box-shadow: none;
  }
  .st-done {
    background: var(--ok);
    box-shadow: none;
  }
  .go {
    flex: none;
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    margin-right: 2px;
    border-radius: 6px;
    color: var(--accent);
    opacity: 0;
  }
  .iss:hover .go {
    opacity: 1;
  }
  .go:hover {
    background: var(--accent-mist);
  }
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 60;
    background: var(--scrim);
  }
  .modal {
    position: fixed;
    z-index: 61;
    top: 10vh;
    left: 50%;
    transform: translateX(-50%);
    width: min(720px, 90vw);
    max-height: 78vh;
    display: flex;
    flex-direction: column;
    border-radius: var(--r-lg);
    background: var(--paper);
    box-shadow: var(--shadow-sheet);
  }
  .modal header,
  .modal footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 16px;
  }
  .modal footer {
    justify-content: flex-end;
    border-top: 1px solid var(--rule-soft);
  }
  .modal .body {
    flex: 1;
    overflow-y: auto;
    padding: 0 18px 14px;
    font-size: var(--t-sm);
  }
</style>
