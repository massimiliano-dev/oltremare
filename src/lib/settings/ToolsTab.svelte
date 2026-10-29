<script lang="ts">
  import { onMount } from "svelte";
  import Icon from "../Icon.svelte";
  import { api, errorText, type McpServer, type MemoryInfo } from "../api";
  import { settings } from "../state/settings.svelte";
  import { mcp } from "../state/mcp.svelte";
  import { ui } from "../state/ui.svelte";

  const s = $derived(settings.s!);
  let editors = $state<{ name: string; path: string }[]>([]);
  let editingId = $state<string | null>(null);
  let draft = $state<McpServer | null>(null);
  let secretText = $state("");
  let secrets = $state<Record<string, { headers: boolean; env: boolean }>>({});
  let importing = $state(false);
  let mem = $state<MemoryInfo | null>(null);

  async function loadMemory() {
    mem = await api.memoryInfo().catch(() => null);
  }

  async function pickMemory() {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const r = await open({ directory: true, title: "Cartella della memoria (un vault di Obsidian va benissimo)" });
      if (typeof r === "string") {
        await settings.save({ memoryPath: r, memoryEnabled: true });
        await loadMemory();
      }
    } catch {}
  }

  async function toggleMemory(on: boolean) {
    await settings.save({ memoryEnabled: on });
    await loadMemory();
  }

  onMount(async () => {
    loadMemory();
    editors = await api.editors().catch(() => []);
    for (const x of s.mcpServers) secrets[x.id] = await api.mcpSecretHas(x.id).catch(() => ({ headers: false, env: false }));
  });

  const statusOf = (id: string) => mcp.status.find((x) => x.id === id);

  async function importClaude() {
    importing = true;
    try {
      const added = await api.mcpImportClaude();
      ui.toast(added.length ? `Importati da Claude Code: ${added.join(", ")}` : "Nessun server nuovo in Claude Code");
    } catch (e) {
      ui.toast(errorText(e), "error");
    } finally {
      importing = false;
    }
  }

  function startNew() {
    draft = { id: crypto.randomUUID(), name: "", kind: "http", url: "", headersHelper: "", command: "", args: [], enabled: true };
    editingId = "new";
    secretText = "";
  }
  function startEdit(x: McpServer) {
    draft = structuredClone($state.snapshot(x)) as McpServer;
    editingId = x.id;
    secretText = "";
  }

  async function saveDraft() {
    if (!draft || !draft.name.trim()) return;
    const d = $state.snapshot(draft) as McpServer;
    d.name = d.name.trim();
    try {
      if (secretText.trim()) {
        await api.mcpSecretSet(d.id, d.kind === "http" ? "headers" : "env", secretText.trim());
        secrets[d.id] = await api.mcpSecretHas(d.id);
      }
      const list = editingId === "new" ? [...s.mcpServers, d] : s.mcpServers.map((x) => (x.id === d.id ? d : x));
      await settings.save({ mcpServers: list });
      editingId = null;
      draft = null;
    } catch (e) {
      ui.toast(errorText(e), "error");
    }
  }

  async function remove(x: McpServer) {
    await api.mcpSecretSet(x.id, "headers", "").catch(() => {});
    await api.mcpSecretSet(x.id, "env", "").catch(() => {});
    await settings.save({ mcpServers: s.mcpServers.filter((y) => y.id !== x.id) });
  }

  function toggle(x: McpServer, on: boolean) {
    settings.save({ mcpServers: s.mcpServers.map((y) => (y.id === x.id ? { ...y, enabled: on } : y)) });
  }
</script>

<section>
  <div class="head">
    <h3>Memoria</h3>
    {#if mem?.ready}
      <button class="btn btn-quiet sm" onclick={() => api.memoryOpen().catch((e) => ui.toast(errorText(e), "error"))}><Icon name="book" size={13} /> {mem.obsidian ? "Apri in Obsidian" : "Apri la cartella"}</button>
    {/if}
  </div>
  <p class="help">Gli agenti ricordano in note Markdown collegate tra loro, che puoi leggere e modificare anche con Obsidian: persone, progetti, preferenze, decisioni. Il briefing finisce nella nota del giorno e ogni progetto dello Studio ha una nota con com'è fatto, le decisioni e il diario dei lavori. Le note non si cancellano mai: se un agente ne riscrive una, la versione vecchia resta nella cartella .trash.</p>
  <div class="srv" class:off={!s.memoryEnabled}>
    <input type="checkbox" checked={s.memoryEnabled} onchange={(e) => toggleMemory((e.target as HTMLInputElement).checked)} aria-label="Memoria attiva" />
    <span class="dot" class:ok={mem?.ready}></span>
    <span class="who">
      <strong>{s.memoryPath ? s.memoryPath.replace(/^\/Users\/[^/]+/, "~") : "Nessuna cartella scelta"}</strong>
      <small>
        {#if !s.memoryEnabled}Spenta: gli agenti non ricordano nulla tra una conversazione e l'altra
        {:else if !s.memoryPath}Scegli una cartella{mem?.suggested ? `: ho trovato ${mem.suggested.replace(/^\/Users\/[^/]+/, "~")}` : ""}
        {:else if !mem?.ready}La cartella non esiste più: scegline un'altra
        {:else}{mem.notes} {mem.notes === 1 ? "nota" : "note"} · gli agenti scrivono senza chiedere{/if}
      </small>
    </span>
    {#if !s.memoryPath && mem?.suggested}
      <button class="btn btn-quiet sm" onclick={async () => { await settings.save({ memoryPath: mem!.suggested!, memoryEnabled: true }); loadMemory(); }}>Usa questa</button>
    {/if}
    <button class="btn btn-line sm" onclick={pickMemory}><Icon name="folder" size={13} /> {s.memoryPath ? "Cambia" : "Scegli"}</button>
  </div>
</section>

<section>
  <div class="head">
    <h3>Server MCP</h3>
    {#if s.mcpServers.length}
      <button class="btn btn-quiet sm" onclick={() => mcp.reload()} disabled={mcp.reloading}><Icon name="refresh" size={13} /> {mcp.reloading ? "Mi collego…" : "Ricollega"}</button>
    {/if}
  </div>
  <p class="help">Collega servizi esterni (gestione delle issue, database, strumenti di lavoro): i loro strumenti diventano strumenti degli agenti, con le stesse regole di approvazione. Le istruzioni del servizio arrivano all'agente che li usa.</p>

  {#each s.mcpServers as x (x.id)}
    {@const st = statusOf(x.id)}
    {#if editingId === x.id && draft}
      {@render form()}
    {:else}
      <div class="srv" class:off={!x.enabled}>
        <input type="checkbox" checked={x.enabled} onchange={(e) => toggle(x, (e.target as HTMLInputElement).checked)} aria-label="Attivo" />
        <span class="dot" class:ok={st?.connected} class:bad={!!st?.error} class:live={st?.connecting}></span>
        <span class="who">
          <strong>{x.name}</strong>
          <small>
            {#if !x.enabled}Spento{:else if st?.connecting}Mi collego…{:else if st?.connected}{st.tools.length} strumenti{:else if st?.error}{st.error}{:else}Non collegato{/if}
            · {x.kind === "http" ? x.url.replace(/^https?:\/\//, "").split("/")[0] : x.command}
          </small>
        </span>
        <button class="icon-btn" onclick={() => startEdit(x)} aria-label="Modifica {x.name}"><Icon name="pen" size={14} /></button>
        <button class="icon-btn" onclick={() => remove(x)} aria-label="Rimuovi {x.name}"><Icon name="x" size={14} /></button>
      </div>
    {/if}
  {/each}

  {#if editingId === "new" && draft}
    {@render form()}
  {:else}
    <div class="row-btns">
      <button class="btn btn-line" onclick={importClaude} disabled={importing}><Icon name="download" size={14} /> {importing ? "Importo…" : "Importa da Claude Code"}</button>
      <button class="btn btn-quiet" onclick={startNew}><Icon name="plus" size={14} /> Aggiungi a mano</button>
    </div>
    <small class="help">«Importa» copia i server che hai già configurato in Claude Code (ambito utente), con lo stesso modo di autenticarsi. Le intestazioni fisse finiscono nel portachiavi.</small>
  {/if}
</section>

{#snippet form()}
  {#if draft}
    <div class="form">
      <div class="two">
        <label class="field"><span>Nome</span><input class="input" bind:value={draft.name} placeholder="es. issue" spellcheck="false" /></label>
        <label class="field">
          <span>Tipo</span>
          <select class="input" bind:value={draft.kind}>
            <option value="http">Indirizzo web (HTTP)</option>
            <option value="stdio">Programma sul computer</option>
          </select>
        </label>
      </div>
      {#if draft.kind === "http"}
        <label class="field"><span>Indirizzo</span><input class="input" bind:value={draft.url} placeholder="https://…/mcp" spellcheck="false" /></label>
        <label class="field">
          <span>Comando per le intestazioni (facoltativo)</span>
          <input class="input mono" bind:value={draft.headersHelper} placeholder='cat "$HOME/.config/servizio/headers.json"' spellcheck="false" />
          <small class="help">Deve stampare un oggetto JSON di intestazioni HTTP. Si riesegue quando l'accesso scade.</small>
        </label>
      {:else}
        <label class="field"><span>Comando</span><input class="input mono" bind:value={draft.command} placeholder="npx" spellcheck="false" /></label>
        <label class="field">
          <span>Argomenti</span>
          <input class="input mono" value={draft.args.join(" ")} onchange={(e) => draft && (draft.args = (e.target as HTMLInputElement).value.split(" ").filter(Boolean))} placeholder="-y @modelcontextprotocol/server-…" spellcheck="false" />
        </label>
      {/if}
      <label class="field">
        <span>{draft.kind === "http" ? "Intestazioni fisse" : "Variabili d'ambiente"} (JSON, nel portachiavi)</span>
        <input
          class="input mono"
          type="password"
          bind:value={secretText}
          placeholder={secrets[draft.id]?.[draft.kind === "http" ? "headers" : "env"] ? "Salvate · scrivi per sostituirle" : draft.kind === "http" ? '{"Authorization": "Bearer …"}' : '{"API_KEY": "…"}'}
          spellcheck="false"
          autocomplete="off"
        />
      </label>
      <div class="row-btns">
        <button class="btn btn-primary" onclick={saveDraft} disabled={!draft.name.trim() || (draft.kind === "http" ? !draft.url.trim() : !draft.command.trim())}>Salva e collega</button>
        <button class="btn btn-quiet" onclick={() => { editingId = null; draft = null; }}>Annulla</button>
      </div>
    </div>
  {/if}
{/snippet}

<section>
  <h3>Sviluppo</h3>
  <p class="help">Lo Sviluppatore affida il codice a Claude Code nella cartella del progetto, con il tuo abbonamento: valgono le istruzioni del progetto (CLAUDE.md) e i server MCP configurati in Claude Code. Avvia backend e frontend e prova nel browser. Non fa push.</p>
  <div class="field">
    <span>Dove lavora lo Sviluppatore</span>
    <div class="seg" role="radiogroup" aria-label="Dove lavora lo Sviluppatore">
      <button role="radio" aria-checked={s.devMode !== "oltremare"} class:on={s.devMode !== "oltremare"} onclick={() => settings.save({ devMode: "editor" })}>
        <strong>Nell'editor</strong><small>scrive a Claude nel pannello, come te; se non riesce, passa a Oltremare</small>
      </button>
      <button role="radio" aria-checked={s.devMode === "oltremare"} class:on={s.devMode === "oltremare"} onclick={() => settings.save({ devMode: "oltremare" })}>
        <strong>In Oltremare</strong><small>Claude in background, seguito nella vista Sviluppo (⌘⇧D)</small>
      </button>
    </div>
  </div>
  <label class="field">
    <span>Editor di codice</span>
    <select class="input" value={s.editorApp} onchange={(e) => settings.save({ editorApp: (e.target as HTMLSelectElement).value })}>
      <option value="">{editors[0] ? `Il più usato (${editors[0].name})` : "Nessun editor trovato"}</option>
      {#each editors as ed (ed.path)}<option value={ed.path}>{ed.name}</option>{/each}
    </select>
    <small class="help">Gli agenti aprono qui progetti e file. Valgono VS Code e le app derivate.</small>
  </label>
</section>

<style>
  section {
    display: grid;
    gap: 12px;
    padding: 20px 0 22px;
    border-bottom: 1px solid var(--rule-soft);
  }
  section:last-child {
    border-bottom: 0;
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  h3 {
    margin: 0;
    font-size: var(--t-xs);
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--accent);
  }
  .sm {
    height: 28px;
    padding: 0 10px;
    font-size: var(--t-xs);
    gap: 5px;
  }
  .srv {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 8px 9px 12px;
    border-radius: var(--r-md);
    box-shadow: inset 0 0 0 1px var(--rule);
  }
  .srv.off {
    opacity: 0.6;
  }
  .srv input {
    accent-color: var(--accent);
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--ink-3);
    flex: none;
  }
  .dot.ok {
    background: var(--ok);
  }
  .dot.bad {
    background: var(--danger);
  }
  .dot.live {
    background: var(--accent);
    animation: pulse 1s infinite alternate;
  }
  @keyframes pulse {
    to {
      opacity: 0.3;
    }
  }
  .who {
    flex: 1;
    min-width: 0;
    display: grid;
  }
  .who strong {
    font-weight: 600;
  }
  .who small {
    font-size: var(--t-xs);
    color: var(--ink-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .form {
    display: grid;
    gap: 10px;
    padding: 14px;
    border-radius: var(--r-md);
    background: var(--paper-2);
  }
  .two {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }
  .mono {
    font-family: var(--font-mono, ui-monospace, monospace);
    font-size: var(--t-sm);
  }
  .row-btns {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
</style>
