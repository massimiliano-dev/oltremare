<script lang="ts">
  // Controllo sorgente dello Studio: ramo, sincronizzazione, commit (anche suggerito da Claude), file pronti e da preparare.
  import { fly, slide } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import Icon from "../Icon.svelte";
  import FileIcon from "../dev/FileIcon.svelte";
  import { api, type GitFile } from "../api";
  import { getContext } from "svelte";
  import type { Workspace as WS } from "../state/workspace.svelte";

  const ws = getContext<WS>("ws");

  let menu = $state(false);
  let branches = $state<{ name: string; current: boolean; when: string; upstream: string }[]>([]);
  let log = $state<{ hash: string; author: string; when: string; subject: string; refs: string }[]>([]);
  let filter = $state("");
  let amend = $state(false);
  let armed = $state<string | null>(null);
  let openStaged = $state(true);
  let openChanged = $state(true);
  let armTimer: ReturnType<typeof setTimeout> | undefined;

  const s = $derived(ws.git.state);
  const total = $derived(ws.git.staged.length + ws.git.changed.length);
  const shown = $derived(branches.filter((b) => b.name.toLowerCase().includes(filter.trim().toLowerCase())));
  const newName = $derived(filter.trim().replace(/\s+/g, "-"));
  const canCreate = $derived(!!newName && !branches.some((b) => b.name === newName));
  const canCommit = $derived(!ws.git.busy && (amend || (!!ws.git.message.trim() && total > 0)));

  const LABEL: Record<string, string> = { M: "Modificato", A: "Aggiunto", D: "Eliminato", R: "Rinominato", C: "Copiato", U: "In conflitto", "?": "Nuovo, non tracciato" };

  async function openMenu() {
    menu = !menu;
    if (!menu || !ws.git.project) return;
    filter = "";
    const [b, l] = await Promise.all([api.gitBranches(ws.git.project).catch(() => ({ branches: [] })), api.gitLog(ws.git.project).catch(() => [])]);
    branches = b.branches;
    log = l.slice(0, 12);
  }

  function arm(key: string, run: () => void) {
    if (armed !== key) {
      armed = key;
      clearTimeout(armTimer);
      armTimer = setTimeout(() => (armed = null), 3500);
      return;
    }
    armed = null;
    run();
  }

  function select(f: GitFile, staged: boolean) {
    ws.git.showDiff(f.path, staged);
  }

  function badge(st: string) {
    return st === "?" ? "N" : st;
  }

  function keydown(e: KeyboardEvent) {
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey) && canCommit) {
      e.preventDefault();
      ws.git.commit({ amend });
      amend = false;
    }
  }

  function outside(node: HTMLElement) {
    const h = (e: PointerEvent) => {
      if (!node.contains(e.target as Node)) menu = false;
    };
    const k = (e: KeyboardEvent) => e.key === "Escape" && (menu = false);
    setTimeout(() => window.addEventListener("pointerdown", h));
    window.addEventListener("keydown", k);
    return {
      destroy() {
        window.removeEventListener("pointerdown", h);
        window.removeEventListener("keydown", k);
      },
    };
  }
</script>

{#snippet row(f: GitFile, staged: boolean)}
  {@const nome = f.rel.split("/").pop() ?? f.rel}
  {@const cart = f.rel.includes("/") ? f.rel.slice(0, f.rel.lastIndexOf("/")) : ""}
  {@const on = ws.dev.tab === "diff" && ws.git.diffFile === f.path && ws.git.diffStaged === staged}
  <div class="f" class:on>
    <button class="fmain" onclick={() => select(f, staged)} title="{f.rel} · {LABEL[f.status] ?? f.status}">
      <FileIcon name={nome} />
      <span class="fn" class:gone={f.status === "D"}>{nome}</span>
      {#if cart}<span class="fd">{cart}</span>{/if}
    </button>
    <span class="facts">
      {#if f.status !== "D"}<button onclick={() => ws.openFile(f.path)} title="Apri nell'editor" aria-label="Apri {nome} nell'editor"><Icon name="draft" size={12} /></button>{/if}
      {#if staged}
        <button onclick={() => ws.git.unstage([f.path])} title="Togli dai pronti" aria-label="Togli {nome} dai pronti"><Icon name="minus" size={13} /></button>
      {:else}
        <button class="danger" class:armed={armed === f.path} onclick={() => arm(f.path, () => ws.git.discard([f.path]))} title={armed === f.path ? "Clicca ancora: le modifiche si perdono (i file nuovi vanno nel Cestino)" : "Annulla le modifiche"} aria-label="Annulla le modifiche a {nome}"><Icon name="undo" size={12} /></button>
        <button onclick={() => ws.git.stage([f.path])} title="Prepara per il commit" aria-label="Prepara {nome}"><Icon name="plus" size={13} /></button>
      {/if}
    </span>
    <em class="st st-{f.status === '?' ? 'n' : f.status}" title={LABEL[f.status] ?? f.status}>{badge(f.status)}</em>
  </div>
{/snippet}

{#if s?.repo}
  <div class="scm">
    <div class="head">
      <div class="brwrap">
        <button class="branch" class:open={menu} onclick={openMenu} aria-haspopup="menu" aria-expanded={menu} title="{s.branch} · rami, stash e cronologia">
          <Icon name="branch" size={13} />
          <span class="bn">{#if s.branch?.includes("/")}<small>{s.branch.slice(0, s.branch.lastIndexOf("/") + 1)}</small>{/if}{s.branch?.split("/").pop() || "nessun ramo"}</span>
          <Icon name="down" size={11} />
        </button>
        {#if menu}
          <div class="menu" use:outside transition:fly={{ y: -6, duration: 160, easing: cubicOut }} role="menu">
            <input class="input sm" placeholder="Cerca o crea un ramo…" bind:value={filter} onkeydown={(e) => e.key === "Enter" && canCreate && !shown.length && (ws.git.checkout(newName, true), (menu = false))} />
            <div class="list">
              {#each shown as b (b.name)}
                <button class="mi" class:cur={b.current} onclick={() => { if (!b.current) ws.git.checkout(b.name); menu = false; }} role="menuitem">
                  <span class="dot" class:cur={b.current}>{#if b.current}<Icon name="check" size={10} stroke={2.4} />{/if}</span>
                  <span class="mn">{b.name}</span>
                  <span class="mw">{b.when}</span>
                </button>
              {/each}
              {#if canCreate}
                <button class="mi new" onclick={() => { ws.git.checkout(newName, true); menu = false; }} role="menuitem">
                  <span class="dot"><Icon name="plus" size={10} stroke={2.2} /></span>
                  <span class="mn">Crea il ramo <b>{newName}</b></span>
                </button>
              {/if}
            </div>
            <div class="msep"></div>
            <button class="mi" disabled={!total} onclick={() => { ws.git.stash(); menu = false; }} role="menuitem">
              <span class="dot"><Icon name="archive" size={11} /></span><span class="mn">Metti da parte le modifiche</span>
            </button>
            <button class="mi" disabled={!s.stash} onclick={() => { ws.git.stash(true); menu = false; }} role="menuitem">
              <span class="dot"><Icon name="download" size={11} /></span><span class="mn">Riprendi dallo stash</span>{#if s.stash}<span class="mw">{s.stash}</span>{/if}
            </button>
            {#if log.length}
              <div class="msep"></div>
              <div class="mh"><Icon name="clock" size={11} /> Cronologia</div>
              <ol class="log">
                {#each log as c, i (c.hash)}
                  <li class:first={i === 0}>
                    <span class="node"></span>
                    <span class="lsub" title={c.subject}>{c.subject}</span>
                    <span class="lmeta"><code>{c.hash}</code> · {c.author} · {c.when}</span>
                  </li>
                {/each}
              </ol>
            {/if}
          </div>
        {/if}
      </div>
      <div class="sync">
        <button onclick={() => ws.git.sync("pull")} disabled={!!ws.git.busy || !s.upstream} class:has={!!s.behind} title={s.upstream ? `Aggiorna da ${s.upstream}` : "Nessun ramo remoto collegato"} aria-label="Aggiorna dal remoto">
          <Icon name="arrowDown" size={12} />{#if s.behind}<b>{s.behind}</b>{/if}
        </button>
        <button onclick={() => ws.git.sync("push")} disabled={!!ws.git.busy} class:has={!!s.ahead || !s.upstream} title={s.upstream ? `Invia a ${s.upstream}` : "Pubblica il ramo sul remoto"} aria-label="Invia al remoto">
          <Icon name="arrowUp" size={12} />{#if s.ahead}<b>{s.ahead}</b>{/if}
        </button>
        <button onclick={() => ws.git.sync("fetch")} disabled={!!ws.git.busy} class:spin={ws.git.busy === "fetch" || ws.git.busy === "pull" || ws.git.busy === "push"} title="Controlla il remoto" aria-label="Controlla il remoto">
          <Icon name="sync" size={12} />
        </button>
      </div>
    </div>

    <div class="commit" class:amend>
      <div class="box">
        <textarea
          rows="2"
          bind:value={ws.git.message}
          onkeydown={keydown}
          placeholder={amend ? "Nuovo messaggio (vuoto = lo stesso)" : total ? "Messaggio del commit" : "Nessuna modifica"}
          aria-label="Messaggio del commit"
        ></textarea>
        <button class="ai" class:busy={ws.git.busy === "suggest"} disabled={!total || !!ws.git.busy} onclick={() => ws.git.suggest()} title="Scrivi il messaggio con Claude" aria-label="Suggerisci il messaggio">
          <Icon name="spark" size={13} />
        </button>
      </div>
      <div class="crow">
        <button class="btn btn-primary cbtn" disabled={!canCommit} onclick={() => { ws.git.commit({ amend }); amend = false; }} title="⌘↵">
          {#if ws.git.busy === "commit"}<span class="spinner"></span>{:else}<Icon name="commit" size={14} />{/if}
          {amend ? "Correggi" : "Commit"}{#if !amend && total}<span class="cnt">{ws.git.staged.length || total}</span>{/if}
        </button>
        <button class="btn btn-line pbtn" disabled={!canCommit} onclick={() => { ws.git.commit({ amend, push: true }); amend = false; }} title="Commit e invia al remoto" aria-label="Commit e invia">
          <Icon name="arrowUp" size={13} />
        </button>
      </div>
      <label class="amendt"><input type="checkbox" bind:checked={amend} /> Correggi l'ultimo commit</label>
      {#if !ws.git.staged.length && ws.git.changed.length && !amend}<p class="tip">Nessun file pronto: il commit li includerà tutti.</p>{/if}
    </div>

    {#if ws.git.staged.length}
      <div class="grp">
        <button class="gh" onclick={() => (openStaged = !openStaged)} aria-expanded={openStaged}>
          <span class="chev" class:open={openStaged}><Icon name="chevron" size={10} /></span> Pronte <span class="pill ok">{ws.git.staged.length}</span>
        </button>
        <span class="gacts">
          <button onclick={() => ws.git.showDiff(null, true)} title="Vedi tutte le differenze pronte" aria-label="Vedi differenze pronte"><Icon name="layers" size={12} /></button>
          <button onclick={() => ws.git.unstage([])} title="Togli tutti dai pronti" aria-label="Togli tutti"><Icon name="minus" size={13} /></button>
        </span>
      </div>
      {#if openStaged}
        <div transition:slide={{ duration: 160 }}>{#each ws.git.staged as f (f.path)}{@render row(f, true)}{/each}</div>
      {/if}
    {/if}

    {#if ws.git.changed.length}
      <div class="grp">
        <button class="gh" onclick={() => (openChanged = !openChanged)} aria-expanded={openChanged}>
          <span class="chev" class:open={openChanged}><Icon name="chevron" size={10} /></span> Modifiche <span class="pill warm">{ws.git.changed.length}</span>
        </button>
        <span class="gacts">
          <button onclick={() => ws.git.showDiff(null, false)} title="Vedi tutte le differenze" aria-label="Vedi differenze"><Icon name="layers" size={12} /></button>
          <button class="danger" class:armed={armed === "*"} onclick={() => arm("*", () => ws.git.discard(ws.git.changed.map((f) => f.path)))} title={armed === "*" ? "Clicca ancora: annulla TUTTE le modifiche" : "Annulla tutte le modifiche"} aria-label="Annulla tutte le modifiche"><Icon name="undo" size={12} /></button>
          <button onclick={() => ws.git.stage([])} title="Prepara tutti" aria-label="Prepara tutti"><Icon name="plus" size={13} /></button>
        </span>
      </div>
      {#if openChanged}
        <div transition:slide={{ duration: 160 }}>{#each ws.git.changed as f (f.path)}{@render row(f, false)}{/each}</div>
      {/if}
    {/if}

    {#if !total}
      <div class="clean">
        <span class="ok"><Icon name="check" size={13} stroke={2.2} /></span>
        <span>Tutto in ordine{#if s.last}<small title={s.last}>{s.last}</small>{/if}</span>
      </div>
    {/if}
  </div>
{:else if s && !s.repo}
  <p class="norepo"><Icon name="branch" size={12} /> Cartella senza git: le modifiche non vengono tracciate.</p>
{/if}

<style>
  .scm {
    margin: 10px 2px 6px;
    padding: 8px;
    border-radius: 12px;
    background: color-mix(in oklch, var(--paper) 70%, transparent);
    box-shadow: inset 0 0 0 1px var(--rule-soft);
  }
  .head {
    position: relative;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .brwrap {
    flex: 1;
    min-width: 0;
  }
  .branch {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 8px;
    border-radius: 8px;
    font-size: var(--t-sm);
    font-weight: 650;
    color: var(--ink);
    transition: background-color 0.12s;
  }
  .branch :global(svg:first-child) {
    color: var(--accent);
  }
  .branch:hover,
  .branch.open {
    background: color-mix(in oklch, var(--accent) 9%, transparent);
  }
  .bn {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl;
    text-align: left;
  }
  .bn small {
    font-weight: 500;
    font-size: inherit;
    color: var(--ink-3);
  }
  .sync {
    flex: none;
    display: flex;
    gap: 2px;
  }
  .sync button {
    display: flex;
    align-items: center;
    gap: 2px;
    height: 26px;
    min-width: 26px;
    padding: 0 6px;
    justify-content: center;
    border-radius: 7px;
    color: var(--ink-3);
    font-size: 0.68rem;
    transition: background-color 0.12s, color 0.12s;
  }
  .sync button:hover:not(:disabled) {
    background: var(--paper-3);
    color: var(--ink);
  }
  .sync button:disabled {
    opacity: 0.4;
  }
  .sync button.has {
    color: var(--accent);
    background: color-mix(in oklch, var(--accent) 10%, transparent);
  }
  .sync b {
    font-weight: 750;
    font-variant-numeric: tabular-nums;
  }
  .sync .spin :global(svg) {
    animation: spin 0.9s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .menu {
    position: absolute;
    z-index: 20;
    top: calc(100% + 6px);
    left: -2px;
    right: -2px;
    max-height: 70vh;
    overflow: auto;
    padding: 6px;
    border-radius: 12px;
    background: var(--paper);
    box-shadow:
      0 0 0 1px var(--rule),
      0 12px 32px -8px oklch(0.25 0.06 var(--tint-h) / 0.28);
  }
  .menu .input {
    width: 100%;
    margin-bottom: 4px;
  }
  .list {
    max-height: 220px;
    overflow: auto;
  }
  .mi {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    height: 30px;
    padding: 0 8px;
    border-radius: 7px;
    font-size: var(--t-sm);
    text-align: left;
    color: var(--ink);
  }
  .mi:hover:not(:disabled) {
    background: color-mix(in oklch, var(--accent) 9%, transparent);
  }
  .mi:disabled {
    color: var(--ink-3);
    opacity: 0.55;
  }
  .mi.cur .mn {
    font-weight: 700;
  }
  .mi.new {
    color: var(--accent);
  }
  .dot {
    flex: none;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    color: var(--ink-3);
    background: var(--paper-3);
  }
  .dot.cur {
    color: white;
    background: var(--accent);
  }
  .mn {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .mw {
    flex: none;
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
  .msep {
    height: 1px;
    margin: 6px 4px;
    background: var(--rule-soft);
  }
  .mh {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 2px 8px 4px;
    font-size: var(--t-xs);
    font-weight: 700;
    color: var(--ink-2);
  }
  .log {
    list-style: none;
    margin: 0;
    padding: 0 8px 4px 8px;
  }
  .log li {
    position: relative;
    display: grid;
    padding: 3px 0 5px 18px;
  }
  .log li::before {
    content: "";
    position: absolute;
    left: 4px;
    top: 0;
    bottom: 0;
    width: 1.5px;
    background: color-mix(in oklch, var(--accent) 25%, transparent);
  }
  .log li.first::before {
    top: 9px;
  }
  .log li:last-child::before {
    bottom: calc(100% - 9px);
  }
  .node {
    position: absolute;
    left: 0;
    top: 5px;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--paper);
    box-shadow: inset 0 0 0 2px color-mix(in oklch, var(--accent) 60%, transparent);
  }
  .log li.first .node {
    background: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in oklch, var(--accent) 18%, transparent);
  }
  .lsub {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--t-xs);
    color: var(--ink);
  }
  .lmeta {
    font-size: 0.66rem;
    color: var(--ink-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .lmeta code {
    font-size: 0.66rem;
    color: var(--accent);
  }
  .commit {
    margin-top: 8px;
    display: grid;
    gap: 6px;
  }
  .box {
    position: relative;
  }
  textarea {
    width: 100%;
    min-height: 52px;
    max-height: 160px;
    resize: vertical;
    padding: 7px 30px 7px 9px;
    border-radius: 9px;
    border: 1px solid var(--rule);
    background: var(--paper);
    color: var(--ink);
    font: inherit;
    font-size: var(--t-sm);
    line-height: 1.45;
    field-sizing: content;
    transition: border-color 0.15s, box-shadow 0.15s;
  }
  textarea:focus {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in oklch, var(--accent) 15%, transparent);
  }
  textarea::placeholder {
    color: var(--ink-3);
  }
  .commit.amend textarea {
    border-color: var(--warn);
  }
  .ai {
    position: absolute;
    top: 5px;
    right: 5px;
    width: 22px;
    height: 22px;
    display: grid;
    place-items: center;
    border-radius: 6px;
    color: var(--accent);
    transition: background-color 0.12s, transform 0.2s;
  }
  .ai:hover:not(:disabled) {
    background: color-mix(in oklch, var(--accent) 12%, transparent);
    transform: rotate(-12deg) scale(1.08);
  }
  .ai:disabled {
    opacity: 0.35;
  }
  .ai.busy :global(svg) {
    animation: twinkle 0.9s ease-in-out infinite;
  }
  @keyframes twinkle {
    50% {
      transform: scale(0.7) rotate(45deg);
      opacity: 0.5;
    }
  }
  .crow {
    display: flex;
    gap: 4px;
  }
  .cbtn {
    flex: 1;
    justify-content: center;
    gap: 6px;
  }
  .cnt {
    min-width: 18px;
    height: 17px;
    padding: 0 5px;
    border-radius: 999px;
    display: grid;
    place-items: center;
    font-size: 0.65rem;
    font-weight: 750;
    background: oklch(1 0 0 / 0.22);
  }
  .pbtn {
    flex: none;
    width: 34px;
    padding: 0;
    justify-content: center;
  }
  .spinner {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    border: 2px solid oklch(1 0 0 / 0.35);
    border-top-color: white;
    animation: spin 0.7s linear infinite;
  }
  .amendt {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: var(--t-xs);
    color: var(--ink-3);
    cursor: pointer;
  }
  .amendt input {
    accent-color: var(--accent);
  }
  .tip {
    font-size: var(--t-xs);
    color: var(--ink-3);
    line-height: 1.4;
  }
  .grp {
    display: flex;
    align-items: center;
    margin-top: 8px;
  }
  .gh {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 5px;
    height: 24px;
    padding: 0 4px;
    font-size: var(--t-xs);
    font-weight: 700;
    color: var(--ink-2);
  }
  .chev {
    display: grid;
    color: var(--ink-3);
    transition: transform 0.18s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .chev.open {
    transform: rotate(90deg);
  }
  .pill {
    min-width: 18px;
    height: 17px;
    padding: 0 5px;
    border-radius: 999px;
    display: grid;
    place-items: center;
    font-size: 0.62rem;
    font-weight: 750;
  }
  .pill.warm {
    color: var(--warn-ink);
    background: oklch(0.8 0.12 80 / 0.3);
  }
  .pill.ok {
    color: var(--ok);
    background: color-mix(in oklch, var(--ok) 15%, transparent);
  }
  .gacts,
  .facts {
    display: flex;
    gap: 1px;
  }
  .gacts button,
  .facts button {
    width: 22px;
    height: 22px;
    display: grid;
    place-items: center;
    border-radius: 6px;
    color: var(--ink-3);
    transition: background-color 0.12s, color 0.12s;
  }
  .gacts button:hover,
  .facts button:hover {
    background: var(--paper-3);
    color: var(--ink);
  }
  button.danger:hover,
  button.armed {
    color: var(--danger) !important;
    background: var(--danger-wash) !important;
  }
  button.armed {
    animation: nudge 0.35s ease;
  }
  @keyframes nudge {
    25% {
      transform: translateX(-2px);
    }
    75% {
      transform: translateX(2px);
    }
  }
  .f {
    position: relative;
    display: flex;
    align-items: center;
    height: 26px;
    padding-right: 4px;
    border-radius: 8px;
    transition: background-color 0.12s;
  }
  .f:hover {
    background: color-mix(in oklch, var(--accent) 7%, transparent);
  }
  .f.on {
    background: color-mix(in oklch, var(--accent) 15%, transparent);
  }
  .fmain {
    flex: 1;
    min-width: 0;
    height: 100%;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 4px 0 14px;
    text-align: left;
    font-size: var(--t-sm);
  }
  .fn {
    flex: none;
    max-width: 62%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--ink);
  }
  .fn.gone {
    text-decoration: line-through;
    color: var(--ink-3);
  }
  .fd {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl;
    text-align: left;
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
  .facts {
    display: none;
  }
  .f:hover .facts,
  .f:focus-within .facts {
    display: flex;
  }
  .st {
    flex: none;
    width: 16px;
    height: 16px;
    margin-left: 3px;
    border-radius: 5px;
    display: grid;
    place-items: center;
    font-style: normal;
    font-size: 0.6rem;
    font-weight: 800;
  }
  .st-M,
  .st-R,
  .st-C {
    color: var(--warn-ink);
    background: oklch(0.8 0.12 80 / 0.3);
  }
  .st-A,
  .st-n {
    color: var(--ok);
    background: color-mix(in oklch, var(--ok) 16%, transparent);
  }
  .st-D,
  .st-U {
    color: var(--danger);
    background: color-mix(in oklch, var(--danger) 14%, transparent);
  }
  .clean {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 10px;
    padding: 2px 4px;
    font-size: var(--t-xs);
    color: var(--ink-2);
  }
  .clean > span:last-child {
    display: grid;
    min-width: 0;
  }
  .clean small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--ink-3);
  }
  .clean .ok {
    flex: none;
    width: 22px;
    height: 22px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    color: var(--ok);
    background: color-mix(in oklch, var(--ok) 14%, transparent);
  }
  .norepo {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 12px 8px 4px;
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
  @media (prefers-reduced-motion: reduce) {
    .sync .spin :global(svg),
    .ai.busy :global(svg),
    button.armed {
      animation: none;
    }
  }
</style>
