<script lang="ts">
  // Spazio di sviluppo: un piccolo IDE per seguire e guidare Claude su un progetto.
  import { onMount, tick } from "svelte";
  import Icon from "../Icon.svelte";
  import FileTree from "./FileTree.svelte";
  import { dev } from "../state/dev.svelte";
  import { settings } from "../state/settings.svelte";
  import { ui } from "../state/ui.svelte";
  import { markdown } from "../markdown";
  import ChatPanel from "../panels/ChatPanel.svelte";
  import { agents } from "../state/agents.svelte";

  let text = $state("");
  let here = $state(settings.s?.devMode === "oltremare");
  let openItems = $state<Record<number, boolean>>({});
  let chat: HTMLElement | undefined = $state();
  let frame: HTMLIFrameElement | undefined = $state();
  let showProjects = $state(false);
  /** Colonna destra: la conversazione con lo Sviluppatore, oppure quella di Claude sul progetto */
  let side = $state<"agent" | "claude">("agent");

  const name = $derived(dev.folder?.split(/[\\/]/).filter(Boolean).pop() ?? "Nessun progetto");
  const items = $derived(dev.session?.items ?? []);
  const lines = $derived((dev.file?.content ?? "").split("\n"));
  const diffLines = $derived(dev.diff.split("\n"));
  const bottomProc = $derived(dev.processes.find((p) => p.id === dev.bottom) ?? null);

  onMount(() => {
    dev.start();
    return () => dev.stop();
  });

  // La conversazione scende da sola mentre arrivano messaggi (se si è già in fondo)
  let lastLen = 0;
  $effect(() => {
    const n = items.length;
    if (n !== lastLen && chat) {
      const atBottom = chat.scrollHeight - chat.scrollTop - chat.clientHeight < 120;
      lastLen = n;
      if (atBottom) tick().then(() => chat && (chat.scrollTop = chat.scrollHeight));
    }
  });

  async function send(newSession = false) {
    if (await dev.send(text, { newSession, here })) text = "";
  }

  function key(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey && (settings.s?.sendWithEnter || e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      send();
    }
  }

  function diffClass(l: string) {
    if (l.startsWith("+++") || l.startsWith("---") || l.startsWith("diff ") || l.startsWith("index ")) return "meta";
    if (l.startsWith("@@")) return "hunk";
    if (l.startsWith("+")) return "add";
    if (l.startsWith("-")) return "del";
    return "";
  }
</script>

<div class="dev">
  <header class="top">
    <div class="proj">
      <button class="pname" onclick={() => (showProjects = !showProjects)}><Icon name="folder" size={15} /> <strong>{name}</strong> <Icon name="down" size={11} /></button>
      {#if showProjects}
        <div class="menu" role="menu">
          {#each dev.recent as r (r)}
            <button role="menuitem" onclick={() => { showProjects = false; dev.open(r); }}>{r.replace(/^\/Users\/[^/]+/, "~")}</button>
          {/each}
          <button role="menuitem" class="pick" onclick={() => { showProjects = false; dev.pick(); }}><Icon name="plus" size={13} /> Apri una cartella…</button>
        </div>
      {/if}
      {#if dev.git?.repo}
        <span class="branch"><Icon name="layers" size={12} /> {dev.git.branch || "detached"}</span>
        <span class="last" title="Ultimo commit">{dev.git.last}</span>
      {/if}
    </div>
    <div class="acts">
      {#if dev.git?.changes?.length}
        <button class="btn btn-quiet sm" onclick={() => dev.showDiff()}>{dev.git.changes.length} file cambiati</button>
      {/if}
      <button class="btn btn-line sm" onclick={() => (ui.view = "chat")} title="Torna alla chat e agli altri agenti"><Icon name="chat" size={13} /> Tutti gli agenti</button>
    </div>
  </header>

  {#if !dev.folder}
    <div class="empty">
      <Icon name="terminal" size={28} />
      <p>Apri un progetto per seguire il lavoro: file, differenze, Claude dal vivo, processi e anteprima.</p>
      <button class="btn btn-primary" onclick={() => dev.pick()}>Apri una cartella</button>
    </div>
  {:else}
    <div class="grid">
      <aside class="tree">
        {#if dev.git?.changes?.length}
          <div class="sec">Cambiati</div>
          {#each dev.git.changes.slice(0, 40) as c (c.path)}
            <button class="chg" class:on={dev.diffFile === c.path && dev.tab === "diff"} onclick={() => dev.openFile(c.path)} title={c.rel}>
              <em class="st-{c.status === '??' ? 'n' : c.status[0]}">{c.status === "??" ? "N" : c.status[0]}</em><span>{c.rel}</span>
            </button>
          {/each}
          <div class="sec">File</div>
        {/if}
        <FileTree path={dev.folder} />
      </aside>

      <section class="center">
        <div class="tabs">
          <button class:on={dev.tab === "file"} onclick={() => (dev.tab = "file")}>{dev.file ? dev.file.path.split(/[\\/]/).pop() : "File"}</button>
          <button class:on={dev.tab === "diff"} onclick={() => dev.showDiff(dev.diffFile ?? undefined)}>Differenze{dev.diffFile ? ` · ${dev.diffFile.split(/[\\/]/).pop()}` : ""}</button>
          <button class:on={dev.tab === "preview"} onclick={() => (dev.tab = "preview")}>Anteprima{dev.urls.length ? ` · ${dev.urls.length}` : ""}</button>
        </div>
        <div class="pane">
          {#if dev.tab === "file"}
            {#if !dev.file}
              <p class="hint">Scegli un file a sinistra. I file cambiati da Claude sono colorati.</p>
            {:else if dev.file.binary}
              <p class="hint">File binario ({Math.round(dev.file.size / 1024)} KB).</p>
            {:else}
              <pre class="code"><code>{#each lines as l, i (i)}<span class="ln">{i + 1}</span>{l}{"\n"}{/each}</code></pre>
              {#if dev.file.truncated}<p class="hint">Mostrati i primi 512 KB.</p>{/if}
            {/if}
          {:else if dev.tab === "diff"}
            {#if !dev.diff.trim()}
              <p class="hint">Nessuna differenza rispetto all'ultimo commit.</p>
            {:else}
              <pre class="code diff"><code>{#each diffLines as l, i (i)}<span class="dl {diffClass(l)}">{l || " "}</span>{"\n"}{/each}</code></pre>
            {/if}
          {:else}
            {#if dev.urls.length}
              <div class="urlbar">
                <select class="input" bind:value={dev.previewUrl}>
                  {#each dev.urls as u (u)}<option value={u}>{u}</option>{/each}
                </select>
                <button class="icon-btn" title="Ricarica" aria-label="Ricarica" onclick={() => frame && (frame.src = dev.previewUrl ?? "")}><Icon name="refresh" size={15} /></button>
              </div>
              {#if dev.previewUrl}<iframe bind:this={frame} src={dev.previewUrl} title="Anteprima"></iframe>{/if}
            {:else}
              <p class="hint">Nessun server acceso. Quando l'agente (o tu, chiedendo a Claude) avvia il backend o il frontend, l'indirizzo locale compare qui.</p>
            {/if}
          {/if}
        </div>
        {#if dev.processes.length}
          <div class="procs">
            <div class="ptabs">
              {#each dev.processes as p (p.id)}
                <button class:on={dev.bottom === p.id} onclick={() => (dev.bottom = dev.bottom === p.id ? null : p.id)}>
                  <span class="led" class:run={p.running} class:bad={!p.running && p.exitCode !== 0}></span>{p.name}
                </button>
              {/each}
            </div>
            {#if bottomProc}
              <pre class="log">{bottomProc.log || "(nessun output)"}</pre>
            {/if}
          </div>
        {/if}
      </section>

      <section class="claude">
        <div class="sidetabs" role="tablist">
          <button role="tab" aria-selected={side === "agent"} class:on={side === "agent"} onclick={() => (side = "agent")}>{agents.selected?.name ?? "Sviluppatore"}</button>
          <button role="tab" aria-selected={side === "claude"} class:on={side === "claude"} onclick={() => (side = "claude")}>
            Claude sul progetto{#if dev.session?.running}<span class="pulse"></span>{/if}
          </button>
        </div>
        {#if side === "agent"}
          <div class="agentchat"><ChatPanel /></div>
        {:else}
        <div class="chead">
          <strong>Claude</strong>
          {#if dev.session?.running}<span class="live"><span class="pulse"></span>{dev.session.stuck ? "fermo: forse aspetta un permesso" : "al lavoro"}</span>{/if}
          {#if dev.session?.sessions.length}
            <select class="input sm" value={dev.session?.id ?? ""} onchange={(e) => { dev.sessionId = (e.target as HTMLSelectElement).value; dev.refreshSession(); }} aria-label="Conversazione">
              {#each dev.session.sessions as s (s.id)}<option value={s.id}>{s.title || s.id.slice(0, 8)}</option>{/each}
            </select>
          {/if}
          {#if dev.session?.running && dev.session.here}
            <button class="icon-btn" title="Ferma Claude" aria-label="Ferma Claude" onclick={() => dev.stopClaude()}><Icon name="stop" size={14} /></button>
          {/if}
        </div>
        <div class="msgs" bind:this={chat}>
          {#each items as it, i (i)}
            {#if it.kind === "user"}
              <div class="u">{it.text}</div>
            {:else if it.kind === "text"}
              <div class="md a">{@html markdown(it.text ?? "")}</div>
            {:else}
              <button class="tool" class:err={it.error} onclick={() => (openItems[i] = !openItems[i])}>
                {#if !it.done}<span class="spin"></span>{:else if it.error}<Icon name="x" size={12} />{:else}<Icon name="check" size={12} />{/if}
                <strong>{it.tool}</strong><span>{it.title}</span>
              </button>
              {#if openItems[i] && it.result}<pre class="res">{it.result}</pre>{/if}
            {/if}
          {:else}
            <p class="hint">Qui vedi la conversazione di Claude su questo progetto, sia nell'editor sia in Oltremare.</p>
          {/each}
        </div>
        <div class="composer">
          <textarea class="input" rows="3" bind:value={text} onkeydown={key} placeholder="Scrivi a Claude su {name}…"></textarea>
          <div class="crow">
            <div class="seg2" role="radiogroup" aria-label="Dove">
              <button role="radio" aria-checked={!here} class:on={!here} onclick={() => (here = false)} title="Nel pannello di Claude dell'editor">Nell'editor</button>
              <button role="radio" aria-checked={here} class:on={here} onclick={() => (here = true)} title="Claude in background, seguito qui">Qui</button>
            </div>
            <button class="link" onclick={() => send(true)} disabled={!text.trim() || dev.sending}>Nuova conversazione</button>
            <button class="btn btn-primary sm" onclick={() => send()} disabled={!text.trim() || dev.sending}>{dev.sending ? "Invio…" : "Invia"}</button>
          </div>
        </div>
        {/if}
      </section>
    </div>
  {/if}
</div>

<style>
  .dev {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    background: var(--paper);
  }
  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    height: 44px;
    padding: 0 12px;
    border-bottom: 1px solid var(--rule-soft);
  }
  .proj {
    position: relative;
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
  }
  .pname {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 8px;
    border-radius: 6px;
  }
  .pname:hover {
    background: var(--paper-2);
  }
  .menu {
    position: absolute;
    top: 34px;
    left: 0;
    z-index: 20;
    min-width: 340px;
    padding: 6px;
    border-radius: var(--r-md);
    background: var(--paper);
    box-shadow: var(--shadow-sheet);
    display: grid;
  }
  .menu button {
    text-align: left;
    padding: 7px 9px;
    border-radius: 6px;
    font-size: var(--t-sm);
  }
  .menu button:hover {
    background: var(--paper-2);
  }
  .menu .pick {
    display: flex;
    gap: 6px;
    align-items: center;
    color: var(--accent);
  }
  .branch {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: var(--t-xs);
    color: var(--accent);
  }
  .last {
    font-size: var(--t-xs);
    color: var(--ink-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 380px;
  }
  .acts {
    display: flex;
    gap: 8px;
    flex: none;
  }
  .sm {
    height: 28px;
    padding: 0 10px;
    font-size: var(--t-xs);
    gap: 5px;
  }
  .empty {
    flex: 1;
    display: grid;
    place-content: center;
    justify-items: center;
    gap: 12px;
    color: var(--ink-3);
    text-align: center;
  }
  .grid {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 250px minmax(0, 1fr) 460px;
  }
  .tree {
    overflow: auto;
    padding: 8px 6px;
    border-right: 1px solid var(--rule-soft);
    background: var(--paper-2);
  }
  .sec {
    margin: 8px 8px 4px;
    font-size: 0.66rem;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--ink-3);
  }
  .chg {
    width: 100%;
    display: flex;
    gap: 7px;
    align-items: center;
    height: 24px;
    padding: 0 8px;
    border-radius: 5px;
    font-size: var(--t-sm);
    text-align: left;
  }
  .chg:hover {
    background: var(--paper);
  }
  .chg.on {
    background: var(--accent-mist);
  }
  .chg span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl;
    text-align: left;
  }
  .chg em {
    font-style: normal;
    font-size: 0.66rem;
    font-weight: 700;
    width: 10px;
  }
  .st-M,
  .st-R {
    color: oklch(0.6 0.13 70);
  }
  .st-A,
  .st-n {
    color: var(--ok);
  }
  .st-D {
    color: var(--danger);
  }
  .center {
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .tabs {
    display: flex;
    gap: 2px;
    padding: 6px 8px 0;
    border-bottom: 1px solid var(--rule-soft);
  }
  .tabs button {
    padding: 6px 12px;
    font-size: var(--t-sm);
    color: var(--ink-3);
    border-radius: 6px 6px 0 0;
    max-width: 260px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tabs button.on {
    color: var(--ink);
    background: var(--paper-2);
    box-shadow: inset 0 -2px 0 var(--accent);
  }
  .pane {
    flex: 1;
    min-height: 0;
    overflow: auto;
    display: flex;
    flex-direction: column;
  }
  .hint {
    padding: 18px;
    color: var(--ink-3);
    font-size: var(--t-sm);
    line-height: 1.5;
  }
  .code {
    margin: 0;
    padding: 10px 0;
    font-family: var(--font-mono, ui-monospace, "SF Mono", Menlo, monospace);
    font-size: 12.5px;
    line-height: 1.55;
    tab-size: 4;
    white-space: pre;
  }
  .ln {
    display: inline-block;
    width: 52px;
    padding-right: 14px;
    text-align: right;
    color: var(--ink-3);
    opacity: 0.6;
    user-select: none;
  }
  .dl {
    display: inline-block;
    min-width: 100%;
    padding: 0 12px;
  }
  .dl.add {
    background: oklch(0.6 0.14 150 / 0.14);
  }
  .dl.del {
    background: oklch(0.6 0.18 25 / 0.14);
  }
  .dl.hunk {
    color: var(--accent);
  }
  .dl.meta {
    color: var(--ink-3);
    font-weight: 600;
  }
  .urlbar {
    display: flex;
    gap: 6px;
    padding: 8px;
  }
  iframe {
    flex: 1;
    width: 100%;
    border: 0;
    background: white;
  }
  .procs {
    border-top: 1px solid var(--rule-soft);
    background: var(--paper-2);
  }
  .ptabs {
    display: flex;
    gap: 4px;
    padding: 5px 8px;
    flex-wrap: wrap;
  }
  .ptabs button {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 3px 9px;
    border-radius: 999px;
    font-size: var(--t-xs);
    box-shadow: inset 0 0 0 1px var(--rule);
  }
  .ptabs button.on {
    box-shadow: inset 0 0 0 1.5px var(--accent);
    color: var(--accent);
  }
  .led {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--ink-3);
  }
  .led.run {
    background: var(--ok);
  }
  .led.bad {
    background: var(--danger);
  }
  .log {
    margin: 0;
    max-height: 220px;
    overflow: auto;
    padding: 8px 12px;
    font-family: var(--font-mono, ui-monospace, Menlo, monospace);
    font-size: 11.5px;
    line-height: 1.5;
    white-space: pre-wrap;
    background: var(--paper);
  }
  .claude {
    min-height: 0;
    display: flex;
    flex-direction: column;
    border-left: 1px solid var(--rule-soft);
  }
  .sidetabs {
    display: flex;
    gap: 2px;
    padding: 6px 8px 0;
    border-bottom: 1px solid var(--rule-soft);
  }
  .sidetabs button {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    font-size: var(--t-sm);
    color: var(--ink-3);
    border-radius: 6px 6px 0 0;
  }
  .sidetabs button.on {
    color: var(--ink);
    background: var(--paper-2);
    box-shadow: inset 0 -2px 0 var(--accent);
  }
  .agentchat {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .agentchat :global(> *) {
    flex: 1;
    min-height: 0;
  }
  .chead {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--rule-soft);
  }
  .chead select {
    margin-left: auto;
    max-width: 170px;
    height: 28px;
    font-size: var(--t-xs);
  }
  .live {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: var(--t-xs);
    color: var(--accent);
  }
  .pulse {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--accent);
    animation: p 1s infinite alternate;
  }
  @keyframes p {
    to {
      opacity: 0.25;
    }
  }
  .msgs {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .u {
    align-self: flex-end;
    max-width: 90%;
    padding: 8px 11px;
    border-radius: 12px 12px 4px 12px;
    background: var(--accent-mist);
    white-space: pre-wrap;
    font-size: var(--t-sm);
  }
  .a {
    font-size: var(--t-sm);
  }
  .tool {
    display: flex;
    align-items: center;
    gap: 7px;
    width: 100%;
    padding: 4px 6px;
    border-radius: 6px;
    font-size: var(--t-xs);
    color: var(--ink-3);
    text-align: left;
    min-width: 0;
  }
  .tool:hover {
    background: var(--paper-2);
  }
  .tool strong {
    color: var(--ink-2);
    flex: none;
  }
  .tool span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tool.err {
    color: var(--danger);
  }
  .spin {
    width: 11px;
    height: 11px;
    border-radius: 50%;
    border: 2px solid var(--accent-wash);
    border-top-color: var(--accent);
    animation: s 0.8s linear infinite;
    flex: none;
  }
  @keyframes s {
    to {
      transform: rotate(360deg);
    }
  }
  .res {
    margin: 0 0 4px 24px;
    max-height: 220px;
    overflow: auto;
    padding: 8px;
    border-radius: 6px;
    background: var(--paper-2);
    font-size: 11px;
    white-space: pre-wrap;
  }
  .composer {
    padding: 10px 12px 12px;
    border-top: 1px solid var(--rule-soft);
    display: grid;
    gap: 8px;
  }
  .crow {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .crow .link {
    margin-left: auto;
    font-size: var(--t-xs);
  }
  .seg2 {
    display: flex;
    gap: 2px;
    padding: 2px;
    border-radius: 999px;
    background: var(--paper-2);
  }
  .seg2 button {
    padding: 3px 10px;
    border-radius: 999px;
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
  .seg2 button.on {
    background: var(--paper);
    color: var(--accent);
    box-shadow: 0 1px 2px oklch(0 0 0 / 0.08);
  }
  @media (max-width: 1200px) {
    .grid {
      grid-template-columns: 200px minmax(0, 1fr) 380px;
    }
  }
</style>
