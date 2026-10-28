<script lang="ts">
  import { onMount } from "svelte";
  import Icon from "../Icon.svelte";
  import { api } from "../api";
  import { agents } from "../state/agents.svelte";
  import { runs } from "../state/runs.svelte";
  import { settings } from "../state/settings.svelte";
  import { ui } from "../state/ui.svelte";
  import { fileName, fileIcon, fileExt } from "./files";
  import { approvals } from "../state/approvals.svelte";
  import VoiceButton from "../voice/VoiceButton.svelte";
  import { voice } from "../state/voice.svelte";

  let text = $state("");
  let files = $state<string[]>([]);
  let dragging = $state(false);
  let box: HTMLTextAreaElement | undefined = $state();
  let types: string[] = [];
  const busy = $derived(!!runs.active);
  const enterSends = $derived(settings.s?.sendWithEnter ?? true);
  const canSend = $derived((!!text.trim() || files.length > 0) && !runs.sending);
  const pausedQueue = $derived(!!runs.conversationId && !!runs.paused[runs.conversationId] && runs.queued.length > 0);

  function grow() {
    if (!box) return;
    box.style.height = "auto";
    box.style.height = `${Math.min(box.scrollHeight, 260)}px`;
  }

  function add(paths: string[]) {
    const ok = types.length ? paths.filter((p) => types.includes(p.split(".").pop()?.toLowerCase() ?? "")) : paths;
    const skipped = paths.length - ok.length;
    if (skipped) ui.toast(skipped === 1 ? "Questo tipo di file non si può allegare" : `${skipped} file non si possono allegare`, "error");
    files = Array.from(new Set([...files, ...ok])).slice(0, 10);
    box?.focus();
  }

  async function pick() {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const r = await open({
        multiple: true,
        title: "Allega file",
        filters: types.length
          ? [
              { name: "Documenti, fogli e immagini", extensions: types },
              { name: "PDF", extensions: ["pdf"] },
              { name: "Excel e fogli di calcolo", extensions: ["xlsx", "xls", "xlsm", "ods", "csv"] },
            ]
          : undefined,
      });
      if (r) add(Array.isArray(r) ? r : [r]);
    } catch {
      // finestra di dialogo non disponibile
    }
  }

  async function send() {
    if (!canSend) return;
    const sentText = text;
    const sentFiles = files;
    text = "";
    files = [];
    queueMicrotask(grow);
    // L'agente sta lavorando (o ci sono già messaggi in coda): il messaggio aspetta il suo turno
    if (busy || runs.queued.length) {
      if (!runs.enqueue(sentText.trim(), sentFiles)) {
        text = sentText;
        files = sentFiles;
      } else if (!busy && runs.conversationId) runs.resume(runs.conversationId);
      return;
    }
    if (!(await runs.send(sentText.trim(), sentFiles))) {
      text = sentText;
      files = sentFiles;
    }
  }

  function onKey(e: KeyboardEvent) {
    // Campo vuoto e una richiesta in attesa: Invio approva, Esc nega
    const req = approvals.current;
    if (req && !text.trim() && !files.length) {
      const mod = e.metaKey || e.ctrlKey;
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
    if (e.key === "Backspace" && !text && files.length) {
      files = files.slice(0, -1);
      return;
    }
    if (e.key !== "Enter" || e.isComposing) return;
    const mod = e.metaKey || e.ctrlKey;
    if ((enterSends && !e.shiftKey && !mod) || (!enterSends && mod)) {
      e.preventDefault();
      send();
    }
  }

  export function fill(t: string) {
    text = t;
    queueMicrotask(() => {
      grow();
      box?.focus();
    });
  }

  onMount(() => {
    api.attachmentTypes().then((t) => (types = t)).catch(() => {});
    // Trascina i file dal Finder o da Esplora risorse sulla finestra
    let un: (() => void) | undefined;
    import("@tauri-apps/api/webview")
      .then(({ getCurrentWebview }) =>
        getCurrentWebview().onDragDropEvent((e) => {
          const p = e.payload;
          if (p.type === "over" || p.type === "enter") dragging = true;
          else if (p.type === "leave") dragging = false;
          else if (p.type === "drop") {
            dragging = false;
            if (p.paths.length) add(p.paths);
          }
        }),
      )
      .then((f) => (un = f))
      .catch(() => {});
    return () => un?.();
  });
</script>

{#if dragging}
  <div class="drop" aria-hidden="true">
    <div class="drop-card"><Icon name="clip" size={22} /> Rilascia per allegare<small>PDF, Excel, Word, PowerPoint, immagini, XML, CSV, testo</small></div>
  </div>
{/if}

<div class="composer" class:busy>
  {#if runs.queued.length}
    <ol class="queue" aria-label="Messaggi in coda">
      {#each runs.queued as q, i (q.id)}
        <li>
          <span class="qn">{i + 1}</span>
          <span class="qt" title={q.text}>{q.text || `${q.files.length} file`}{q.files.length && q.text ? ` · ${q.files.length} file` : ""}</span>
          <button class="qa" onclick={() => { runs.unqueue(q.id); fill(q.text); files = q.files; }} title="Riporta nel campo per modificarlo" aria-label="Modifica"><Icon name="pen" size={12} /></button>
          {#if busy}<button class="qa" onclick={() => runs.sendNow(q.id)} title="Interrompi il lavoro in corso e invialo subito" aria-label="Invia subito"><Icon name="bolt" size={12} /></button>{/if}
          <button class="qa" onclick={() => runs.unqueue(q.id)} title="Togli dalla coda" aria-label="Togli"><Icon name="x" size={12} /></button>
        </li>
      {/each}
      <li class="qinfo">
        {#if pausedQueue}
          In pausa · <button class="link" onclick={() => runs.conversationId && runs.resume(runs.conversationId)}>Invia ora</button>
        {:else if busy}
          Partono in ordine appena {agents.selected?.name ?? "l'agente"} finisce
        {/if}
      </li>
    </ol>
  {/if}
  {#if files.length}
    <ul class="files">
      {#each files as f (f)}
        <li title={f}>
          <span class="ext"><Icon name={fileIcon(f)} size={14} /></span>
          <span class="fname">{fileName(f)}</span>
          <small>{fileExt(fileName(f))}</small>
          <button onclick={() => (files = files.filter((x) => x !== f))} aria-label="Togli {fileName(f)}"><Icon name="x" size={12} /></button>
        </li>
      {/each}
    </ul>
  {/if}
  <textarea
    id="composer"
    bind:this={box}
    bind:value={text}
    oninput={grow}
    onkeydown={onKey}
    rows="1"
    placeholder={files.length ? "Cosa devo fare con questi file?" : agents.selected ? `Chiedi a ${agents.selected.name}…` : "Scrivi…"}
    spellcheck="true"
  ></textarea>
  <div class="row">
    <button class="attach" onclick={pick} title="Allega file" aria-label="Allega file"><Icon name="clip" size={17} /></button>
    <VoiceButton />
    <span class="hint">
      {#if voice.recording}<em class="listening">Ti ascolto…</em>{:else if voice.transcript}<em class="heard">«{voice.transcript}»</em>{:else}
      {#if enterSends}<kbd>↵</kbd> {busy ? "accoda" : "invia"} · <kbd>⇧</kbd><kbd>↵</kbd> a capo{:else}<kbd>⌘</kbd><kbd>↵</kbd> {busy ? "accoda" : "invia"}{/if}{/if}
    </span>
    {#if busy}
      <button class="send stop" onclick={() => runs.cancel(runs.active!.id)} aria-label="Ferma" title="Ferma (⌘.)"><Icon name="stop" size={14} fill /></button>
      {#if text.trim() || files.length}
        <button class="send" onclick={send} disabled={!canSend} aria-label="Accoda" title="Accoda: parte appena l'agente finisce"><Icon name="arrowUp" size={17} stroke={2} /></button>
      {/if}
    {:else}
      <button class="send" onclick={send} disabled={!canSend} aria-label="Invia"><Icon name="arrowUp" size={17} stroke={2} /></button>
    {/if}
  </div>
</div>

<style>
  .queue {
    list-style: none;
    margin: 0 0 2px;
    padding: 0;
    display: grid;
    gap: 4px;
  }
  .queue li {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    padding: 5px 6px 5px 8px;
    border-radius: var(--r-sm);
    background: var(--accent-mist);
    font-size: var(--t-sm);
  }
  .queue li.qinfo {
    background: none;
    padding: 0 2px;
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
  .queue li.qinfo:empty {
    display: none;
  }
  .qn {
    flex: none;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    font-size: 0.66rem;
    font-weight: 700;
    color: var(--accent);
    background: var(--paper);
  }
  .qt {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .qa {
    flex: none;
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 6px;
    color: var(--ink-3);
  }
  .qa:hover {
    color: var(--accent);
    background: var(--paper);
  }
  .composer {
    display: grid;
    gap: 6px;
    padding: 12px 12px 10px 16px;
    border-radius: var(--r-xl);
    background: var(--paper);
    box-shadow: 0 0 0 1px var(--rule), 0 10px 30px -18px oklch(0.3 0.12 var(--accent-h) / 0.35);
    transition: box-shadow 0.2s;
  }
  .composer:focus-within {
    box-shadow: 0 0 0 1.5px var(--accent), 0 0 0 5px var(--focus-ring), 0 10px 30px -18px oklch(0.3 0.12 var(--accent-h) / 0.35);
  }
  textarea {
    border: 0;
    outline: 0;
    resize: none;
    background: none;
    font-size: var(--t-lg);
    line-height: 1.5;
    min-height: 26px;
    max-height: 260px;
  }
  textarea::placeholder {
    color: var(--ink-3);
  }
  .files {
    list-style: none;
    margin: 0 0 2px -4px;
    padding: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .files li {
    display: flex;
    align-items: center;
    gap: 7px;
    max-width: 260px;
    height: 32px;
    padding: 0 4px 0 5px;
    border-radius: 9px;
    background: var(--paper-2);
    box-shadow: inset 0 0 0 1px var(--rule-soft);
    animation: settle 0.25s var(--ease-out);
  }
  .ext {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 6px;
    color: var(--accent);
    background: var(--accent-mist);
  }
  .fname {
    font-size: var(--t-sm);
    font-weight: 550;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .files small {
    font-size: 0.64rem;
    font-weight: 700;
    letter-spacing: 0.04em;
    color: var(--ink-3);
  }
  .files button {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 6px;
    color: var(--ink-3);
  }
  .files button:hover {
    color: var(--danger);
    background: var(--danger-wash);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .attach {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    margin-left: -6px;
    border-radius: 8px;
    color: var(--ink-3);
  }
  .attach:hover {
    color: var(--accent);
    background: var(--accent-mist);
  }
  .hint {
    flex: 1;
    font-size: var(--t-xs);
    color: var(--ink-3);
    display: inline-flex;
    align-items: center;
    gap: 3px;
  }
  .listening,
  .heard {
    font-style: normal;
    font-weight: 600;
    color: var(--accent);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .heard {
    font-weight: 500;
    color: var(--ink-2);
    font-style: italic;
  }
  .send {
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    border-radius: 50%;
    background: var(--accent);
    color: var(--on-accent);
    transition: background 0.15s, transform 0.1s, opacity 0.15s;
  }
  .send:hover:not(:disabled) {
    background: var(--accent-hi);
  }
  .send:active:not(:disabled) {
    transform: scale(0.94);
  }
  .send:disabled {
    opacity: 0.35;
  }
  .send.stop {
    background: var(--ink);
    color: var(--paper);
  }
  .drop {
    position: fixed;
    inset: 0;
    z-index: 90;
    display: grid;
    place-items: center;
    background: oklch(0.5 0.15 var(--accent-h) / 0.12);
    backdrop-filter: blur(2px);
    pointer-events: none;
    animation: settle 0.15s;
  }
  .drop-card {
    display: grid;
    justify-items: center;
    gap: 6px;
    padding: 28px 40px;
    border-radius: var(--r-xl);
    border: 2px dashed var(--accent);
    background: var(--paper);
    color: var(--accent);
    font-family: var(--font-serif);
    font-size: 1.5rem;
  }
  .drop-card small {
    font-family: var(--font-sans);
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
</style>
