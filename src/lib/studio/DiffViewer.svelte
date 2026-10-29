<script lang="ts">
  // Differenze leggibili: una scheda per file con +/−, numeri di riga, parole cambiate in risalto, vista unica o affiancata.
  import Icon from "../Icon.svelte";
  import FileIcon from "../dev/FileIcon.svelte";
  import { parseDiff, splitRows, type DFile, type DLine } from "./diff";
  import { getContext } from "svelte";
  import type { Workspace as WS } from "../state/workspace.svelte";

  const ws = getContext<WS>("ws");

  const MAX = 1500;
  const files = $derived(parseDiff(ws.git.diff));
  const totals = $derived(files.reduce((t, f) => ({ add: t.add + f.add, del: t.del + f.del }), { add: 0, del: 0 }));
  let closed = $state<Record<string, boolean>>({});
  let full = $state<Record<string, boolean>>({});
  let armed = $state<string | null>(null);
  let armTimer: ReturnType<typeof setTimeout> | undefined;

  const STATUS: Record<DFile["status"], string> = { modified: "modificato", added: "nuovo", deleted: "eliminato", renamed: "rinominato" };

  function abs(rel: string) {
    return `${ws.project?.path ?? ""}/${rel}`;
  }
  function name(p: string) {
    return p.split("/").pop() ?? p;
  }
  function dir(p: string) {
    return p.includes("/") ? p.slice(0, p.lastIndexOf("/")) : "";
  }
  function lineCount(f: DFile) {
    return f.hunks.reduce((n, h) => n + h.lines.length + 1, 0);
  }
  function discard(f: DFile) {
    if (armed !== f.path) {
      armed = f.path;
      clearTimeout(armTimer);
      armTimer = setTimeout(() => (armed = null), 3500);
      return;
    }
    armed = null;
    ws.git.discard([abs(f.path)]);
  }
  function toggleAll() {
    const anyOpen = files.some((f) => !closed[f.path]);
    closed = Object.fromEntries(files.map((f) => [f.path, anyOpen]));
  }
</script>

{#snippet cell(l: DLine | undefined)}
  {#if l?.segs}{#each l.segs as s, i (i)}{#if s.hot}<mark>{s.text}</mark>{:else}{s.text}{/if}{/each}{:else}{l?.text ?? ""}{/if}
{/snippet}

<div class="dv">
  <header class="bar">
    <div class="seg" role="group" aria-label="Quali modifiche">
      <button class:on={!ws.git.diffStaged} onclick={() => ws.git.showDiff(ws.git.diffFile, false)}>Da preparare <b>{ws.git.changed.length}</b></button>
      <button class:on={ws.git.diffStaged} onclick={() => ws.git.showDiff(ws.git.diffFile, true)}>Pronte <b>{ws.git.staged.length}</b></button>
    </div>
    {#if ws.git.diffFile}
      <button class="chip" onclick={() => ws.git.showDiff(null, ws.git.diffStaged)} title="Mostra tutti i file">
        <FileIcon name={name(ws.git.diffFile)} /> {name(ws.git.diffFile)} <Icon name="x" size={10} />
      </button>
    {/if}
    {#if files.length}
      <span class="tot"><span class="plus">+{totals.add}</span> <span class="minus">−{totals.del}</span> · {files.length} {files.length === 1 ? "file" : "file"}</span>
    {/if}
    <span class="grow"></span>
    {#if files.length > 1}<button class="icon-btn" onclick={toggleAll} title="Apri o chiudi tutti" aria-label="Apri o chiudi tutti"><Icon name="layers" size={14} /></button>{/if}
    <div class="seg" role="group" aria-label="Vista">
      <button class:on={ws.git.view === "unified"} onclick={() => ws.git.setView("unified")} title="Vista unica" aria-label="Vista unica"><Icon name="rows" size={14} /></button>
      <button class:on={ws.git.view === "split"} onclick={() => ws.git.setView("split")} title="Vista affiancata" aria-label="Vista affiancata"><Icon name="split" size={14} /></button>
    </div>
  </header>

  {#if !files.length}
    <div class="empty">
      <span class="ok"><Icon name="check" size={22} /></span>
      <p>{ws.git.diffStaged ? "Niente di pronto per il commit. Prepara i file dalla colonna a sinistra." : "Nessuna modifica da preparare: il progetto è uguale all'ultimo commit."}</p>
    </div>
  {:else}
    <div class="files">
      {#each files as f, fi (fi + f.path)}
        {@const open = !closed[f.path]}
        {@const many = lineCount(f) > MAX && !full[f.path]}
        <section class="file" class:open>
          <div class="fh">
            <button class="fold" onclick={() => (closed[f.path] = open)} aria-expanded={open} aria-label="Apri o chiudi {f.path}">
              <span class="chev" class:open><Icon name="chevron" size={12} /></span>
              <FileIcon name={name(f.path)} />
              <span class="fn">{name(f.path)}</span>
              {#if dir(f.path)}<span class="fd">{dir(f.path)}</span>{/if}
              {#if f.status !== "modified"}<em class="tag tag-{f.status}">{STATUS[f.status]}</em>{/if}
              {#if f.from}<span class="fd">da {f.from}</span>{/if}
            </button>
            <span class="meter" title="+{f.add} −{f.del}">
              <span class="plus">+{f.add}</span><span class="minus">−{f.del}</span>
              <span class="blocks">{#each Array(5) as _, i (i)}{@const tot = f.add + f.del || 1}<i class:a={i < Math.round((f.add / tot) * 5)} class:d={i >= Math.round((f.add / tot) * 5) && f.add + f.del > 0}></i>{/each}</span>
            </span>
            <div class="acts">
              {#if f.status !== "deleted"}<button class="icon-btn" onclick={() => ws.openFile(abs(f.path))} title="Apri nell'editor" aria-label="Apri nell'editor"><Icon name="draft" size={13} /></button>{/if}
              {#if ws.git.diffStaged}
                <button class="icon-btn" onclick={() => ws.git.unstage([abs(f.path)])} title="Togli dai pronti" aria-label="Togli dai pronti"><Icon name="minus" size={14} /></button>
              {:else}
                <button class="icon-btn danger" class:armed={armed === f.path} onclick={() => discard(f)} title={armed === f.path ? "Clicca ancora per annullare le modifiche" : "Annulla le modifiche"} aria-label="Annulla le modifiche">
                  <Icon name="undo" size={13} />{#if armed === f.path}<span>Sicuro?</span>{/if}
                </button>
                <button class="icon-btn" onclick={() => ws.git.stage([abs(f.path)])} title="Prepara per il commit" aria-label="Prepara per il commit"><Icon name="plus" size={14} /></button>
              {/if}
            </div>
          </div>
          {#if open}
            {#if f.binary}
              <p class="note">File binario: le differenze non si possono mostrare.</p>
            {:else if !f.hunks.length}
              <p class="note">{f.status === "renamed" ? "Solo rinominato, contenuto uguale." : "Nessuna riga cambiata (permessi o file vuoto)."}</p>
            {:else if many}
              <p class="note">Differenze lunghe ({lineCount(f)} righe). <button class="link" onclick={() => (full[f.path] = true)}>Mostrale comunque</button></p>
            {:else if ws.git.view === "unified"}
              <div class="scroll">
                <table class="code uni">
                  <tbody>
                    {#each f.hunks as h, hi (hi)}
                      <tr class="hunk"><td class="ln" colspan="2"></td><td class="tx">{h.scope ? h.scope : h.header.replace(/ @@.*/, " @@")}</td></tr>
                      {#each h.lines as l, li (li)}
                        <tr class={l.kind}>
                          <td class="ln">{l.old ?? ""}</td>
                          <td class="ln">{l.new ?? ""}</td>
                          <td class="tx"><span class="sig">{l.kind === "add" ? "+" : l.kind === "del" ? "−" : " "}</span>{@render cell(l)}</td>
                        </tr>
                      {/each}
                    {/each}
                  </tbody>
                </table>
              </div>
            {:else}
              <div class="scroll">
                <table class="code sbs">
                  <colgroup><col class="cln" /><col /><col class="cln" /><col /></colgroup>
                  <tbody>
                    {#each splitRows(f) as r, ri (ri)}
                      {#if r.kind === "hunk"}
                        <tr class="hunk"><td class="ln"></td><td class="tx" colspan="3">{r.hunk.scope ? r.hunk.scope : r.hunk.header.replace(/ @@.*/, " @@")}</td></tr>
                      {:else}
                        <tr>
                          <td class="ln {r.left?.kind ?? 'none'}">{r.left?.old ?? ""}</td>
                          <td class="tx {r.left?.kind ?? 'none'}">{@render cell(r.left)}</td>
                          <td class="ln {r.right?.kind ?? 'none'}">{r.right?.new ?? ""}</td>
                          <td class="tx {r.right?.kind ?? 'none'}">{@render cell(r.right)}</td>
                        </tr>
                      {/if}
                    {/each}
                  </tbody>
                </table>
              </div>
            {/if}
          {/if}
        </section>
      {/each}
    </div>
  {/if}
</div>

<style>
  .dv {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    --add-bg: oklch(0.72 0.13 152 / 0.13);
    --add-hot: oklch(0.72 0.15 152 / 0.38);
    --add-ln: oklch(0.72 0.13 152 / 0.2);
    --del-bg: oklch(0.7 0.15 25 / 0.11);
    --del-hot: oklch(0.7 0.17 25 / 0.32);
    --del-ln: oklch(0.7 0.15 25 / 0.18);
    --plus: oklch(0.5 0.13 152);
    --minus: oklch(0.52 0.17 25);
  }
  :global([data-theme="dark"]) .dv {
    --plus: oklch(0.8 0.14 152);
    --minus: oklch(0.78 0.14 25);
  }
  .bar {
    position: sticky;
    top: 0;
    z-index: 2;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    background: color-mix(in oklch, var(--paper) 92%, transparent);
    backdrop-filter: blur(8px);
    border-bottom: 1px solid var(--rule-soft);
    font-size: var(--t-sm);
  }
  .grow {
    flex: 1;
  }
  .seg {
    display: flex;
    padding: 2px;
    border-radius: 9px;
    background: var(--paper-3);
  }
  .seg button {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 26px;
    padding: 0 10px;
    border-radius: 7px;
    color: var(--ink-3);
    font-size: var(--t-xs);
    font-weight: 600;
    transition: background-color 0.15s, color 0.15s;
  }
  .seg button b {
    font-variant-numeric: tabular-nums;
    font-weight: 750;
    color: var(--ink-2);
  }
  .seg button.on {
    background: var(--paper);
    color: var(--ink);
    box-shadow: 0 1px 3px oklch(0.3 0.05 var(--tint-h) / 0.12);
  }
  .chip {
    display: flex;
    align-items: center;
    gap: 5px;
    height: 26px;
    padding: 0 9px;
    border-radius: 999px;
    font-size: var(--t-xs);
    font-weight: 600;
    color: var(--accent);
    background: color-mix(in oklch, var(--accent) 10%, transparent);
  }
  .tot {
    font-size: var(--t-xs);
    color: var(--ink-3);
    font-variant-numeric: tabular-nums;
  }
  .plus {
    color: var(--plus);
    font-weight: 700;
  }
  .minus {
    color: var(--minus);
    font-weight: 700;
  }
  .empty {
    margin: auto;
    max-width: 340px;
    display: grid;
    justify-items: center;
    gap: 10px;
    text-align: center;
    font-size: var(--t-sm);
    color: var(--ink-3);
    line-height: 1.5;
  }
  .empty .ok {
    width: 46px;
    height: 46px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    color: var(--ok);
    background: color-mix(in oklch, var(--ok) 12%, transparent);
  }
  .files {
    display: grid;
    gap: 10px;
    padding: 12px;
  }
  .file {
    border: 1px solid var(--rule-soft);
    border-radius: 12px;
    background: var(--paper);
    overflow: clip;
    box-shadow: 0 1px 2px oklch(0.3 0.05 var(--tint-h) / 0.05);
  }
  .fh {
    position: sticky;
    top: 43px;
    z-index: 1;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 8px 0 4px;
    min-height: 38px;
    background: var(--paper-2);
  }
  .file.open .fh {
    border-bottom: 1px solid var(--rule-soft);
  }
  .fold {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 7px;
    height: 38px;
    padding: 0 4px;
    text-align: left;
  }
  .chev {
    display: grid;
    color: var(--ink-3);
    transition: transform 0.2s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .chev.open {
    transform: rotate(90deg);
  }
  .fn {
    font-size: var(--t-sm);
    font-weight: 650;
    color: var(--ink);
    white-space: nowrap;
  }
  .fd {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl;
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
  .tag {
    flex: none;
    padding: 1px 7px;
    border-radius: 999px;
    font-style: normal;
    font-size: 0.66rem;
    font-weight: 700;
  }
  .tag-added {
    color: var(--plus);
    background: var(--add-bg);
  }
  .tag-deleted {
    color: var(--minus);
    background: var(--del-bg);
  }
  .tag-renamed {
    color: var(--warn-ink);
    background: oklch(0.8 0.12 80 / 0.25);
  }
  .meter {
    flex: none;
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: var(--t-xs);
    font-variant-numeric: tabular-nums;
  }
  .blocks {
    display: flex;
    gap: 2px;
  }
  .blocks i {
    width: 7px;
    height: 7px;
    border-radius: 2px;
    background: var(--rule);
  }
  .blocks i.a {
    background: var(--plus);
  }
  .blocks i.d {
    background: var(--minus);
  }
  .acts {
    flex: none;
    display: flex;
    gap: 2px;
  }
  .acts .icon-btn {
    width: auto;
    min-width: 26px;
    height: 26px;
    gap: 4px;
    padding: 0 5px;
  }
  .icon-btn.danger:hover,
  .icon-btn.armed {
    color: var(--danger);
    background: var(--danger-wash);
  }
  .icon-btn.armed span {
    font-size: var(--t-xs);
    font-weight: 700;
  }
  .note {
    padding: 14px 16px;
    font-size: var(--t-sm);
    color: var(--ink-3);
  }
  .scroll {
    overflow-x: auto;
    scrollbar-width: thin;
  }
  .code {
    width: 100%;
    border-collapse: collapse;
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
    font-size: 12.5px;
    line-height: 1.6;
  }
  .sbs {
    table-layout: fixed;
  }
  .sbs .cln {
    width: 48px;
  }
  .ln {
    width: 1%;
    min-width: 40px;
    padding: 0 8px;
    text-align: right;
    color: var(--ink-3);
    opacity: 0.75;
    font-size: 11.5px;
    font-variant-numeric: tabular-nums;
    user-select: none;
    vertical-align: top;
    white-space: nowrap;
  }
  .tx {
    padding: 0 14px 0 6px;
    white-space: pre;
    color: var(--ink);
  }
  .sbs .tx {
    white-space: pre-wrap;
    word-break: break-all;
    border-right: 1px solid var(--rule-soft);
  }
  .sbs .tx:last-child {
    border-right: 0;
  }
  .sig {
    display: inline-block;
    width: 14px;
    color: var(--ink-3);
    user-select: none;
  }
  tr.add .tx,
  .tx.add {
    background: var(--add-bg);
  }
  tr.add .ln,
  .ln.add {
    background: var(--add-ln);
    color: var(--plus);
  }
  tr.add .sig {
    color: var(--plus);
  }
  tr.del .tx,
  .tx.del {
    background: var(--del-bg);
  }
  tr.del .ln,
  .ln.del {
    background: var(--del-ln);
    color: var(--minus);
  }
  tr.del .sig {
    color: var(--minus);
  }
  .tx.none,
  .ln.none {
    background: repeating-linear-gradient(-45deg, transparent 0 5px, var(--paper-2) 5px 7px);
  }
  tr.add mark,
  .tx.add mark {
    background: var(--add-hot);
    color: inherit;
    border-radius: 3px;
  }
  tr.del mark,
  .tx.del mark {
    background: var(--del-hot);
    color: inherit;
    border-radius: 3px;
  }
  tr.hunk td {
    padding-block: 3px;
    background: color-mix(in oklch, var(--accent) 6%, transparent);
    color: color-mix(in oklch, var(--accent) 80%, var(--ink-3));
    font-size: 11.5px;
  }
  tr.hunk:not(:first-child) td {
    border-top: 1px solid var(--rule-soft);
  }
</style>
