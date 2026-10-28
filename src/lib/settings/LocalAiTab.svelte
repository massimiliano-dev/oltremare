<script lang="ts">
  import { onMount } from "svelte";
  import Icon from "../Icon.svelte";
  import { local } from "../state/local.svelte";
  import { settings } from "../state/settings.svelte";
  import { fileSize } from "../format";
  import { api } from "../api";
  import { openPath } from "@tauri-apps/plugin-opener";

  let showLog = $state(false);
  const s = $derived(local.status);
  const BACKEND: Record<string, string> = { metal: "Metal (Apple)", cuda: "CUDA (NVIDIA)", vulkan: "Vulkan", cpu: "CPU" };
  const STATE: Record<string, string> = {
    stopped: "Fermo",
    installing: "Installo il motore…",
    starting: "Avvio…",
    loading: "Carico il modello…",
    ready: "Attivo",
    error: "Errore",
  };
  const engine = $derived(local.downloads["motore"]);

  onMount(() => {
    local.refresh();
  });

  async function folder() {
    const p = await api.modelsFolder();
    openPath(p).catch(() => {});
  }
</script>

<section>
  <div class="status st-{s?.state ?? 'stopped'}">
    <span class="led"></span>
    <div class="st-text">
      <strong>{STATE[s?.state ?? "stopped"]}{s?.model && s.state !== "stopped" ? ` · ${local.label(s.model)}` : ""}</strong>
      <small>llama.cpp {s?.build ?? ""} · {BACKEND[s?.backend ?? "cpu"]}{s?.installed ? "" : " · si scarica al primo avvio (circa 15 MB)"}</small>
      {#if s?.message}<small class="msg">{s.message}</small>{/if}
      {#if engine && !engine.done}
        <div class="bar"><span style:transform="scaleX({engine.total ? engine.received / engine.total : 0.05})"></span></div>
      {/if}
    </div>
    {#if s && s.state !== "stopped" && s.state !== "error"}
      <button class="btn btn-line" onclick={() => local.stop()}>Ferma</button>
    {:else if local.installed.length}
      <button class="btn btn-primary" onclick={() => local.start(settings.s?.localModel || local.installed[0].id)}>Avvia</button>
    {/if}
  </div>
  {#if local.hw}
    <p class="hw"><Icon name="cpu" size={14} /> {local.hw.cpu} · {(local.hw.ramTotal / 1e9).toFixed(0)} GB di RAM, {(local.hw.ramAvailable / 1e9).toFixed(0)} GB liberi</p>
  {/if}
  <p class="help">I modelli girano sul tuo computer: gratis, e testi e file non lasciano mai la macchina. Il motore parte da solo quando un agente lo usa.</p>
</section>

<section>
  <h3>Modelli</h3>
  <div class="recs">
    {#each local.llms as m (m.id)}
      {@const p = local.downloads[m.id]}
      {@const fit = local.fit(m)}
      <div class="rec" class:active={settings.s?.localModel === m.id && m.installed}>
        <div class="rec-text">
          <strong>{m.label} {#if m.recommended}<span class="tag">consigliato</span>{/if}{#if m.supportsVision}<span class="tag soft">vede le immagini</span>{/if}</strong>
          <small>{m.note} · {fileSize(m.sizeBytes)}{m.ramNeededGb ? ` · ${m.ramNeededGb} GB di RAM` : ""}</small>
          {#if !m.installed && fit !== "ok"}<small class="fit {fit}">{fit === "tight" ? "Al limite per questo computer" : "Troppo grande per questo computer"}</small>{/if}
          {#if p && !p.done}
            <div class="bar"><span style:transform="scaleX({p.total ? p.received / p.total : 0})"></span></div>
            <small class="pull-status">{fileSize(p.received)} di {fileSize(p.total || m.sizeBytes)}{p.bytesPerSec ? ` · ${fileSize(p.bytesPerSec)}/s` : ""}</small>
          {/if}
        </div>
        {#if p && !p.done}
          <button class="btn btn-quiet" onclick={() => local.cancel(m.id)}>Annulla</button>
        {:else if m.installed}
          {#if settings.s?.localModel === m.id}
            <span class="in-use"><Icon name="check" size={14} /> Predefinito</span>
          {:else}
            <button class="btn btn-line" onclick={() => settings.save({ localModel: m.id })}>Usa</button>
          {/if}
          <button class="icon-btn" title="Elimina" aria-label="Elimina {m.label}" onclick={() => local.remove(m)}><Icon name="trash" size={15} /></button>
        {:else}
          <button class="btn btn-line" disabled={fit === "no"} onclick={() => local.download(m.id)}><Icon name="download" size={14} /> Scarica</button>
        {/if}
      </div>
    {/each}
  </div>
  <div class="row">
    <button class="btn btn-quiet" onclick={() => local.importFile()}><Icon name="plus" size={14} /> Importa un GGUF</button>
    <button class="btn btn-quiet" onclick={folder}><Icon name="folder" size={14} /> Cartella dei modelli</button>
    <button class="btn btn-quiet" onclick={() => { showLog = !showLog; if (showLog) local.loadLog(); }}><Icon name="terminal" size={14} /> Registro</button>
  </div>
  {#if showLog}
    <pre class="log">{local.log.slice(-80).join("\n") || "Nessun messaggio."}</pre>
  {/if}
</section>

<style>
  section {
    display: grid;
    gap: 12px;
    padding: 20px 0 24px;
    border-bottom: 1px solid var(--rule-soft);
  }
  section:last-child {
    border-bottom: 0;
  }
  h3 {
    margin: 0;
    font-size: var(--t-xs);
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--accent);
  }
  .status {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 14px;
    border-radius: var(--r-md);
    background: var(--paper-2);
    box-shadow: inset 0 0 0 1px var(--rule-soft);
  }
  .led {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--ink-3);
    flex: none;
  }
  .st-ready .led {
    background: var(--ok);
    box-shadow: 0 0 0 4px oklch(0.56 0.14 152 / 0.18);
  }
  .st-loading .led,
  .st-starting .led,
  .st-installing .led {
    background: var(--warn);
    animation: blink 1s ease-in-out infinite;
  }
  .st-error .led {
    background: var(--danger);
  }
  @keyframes blink {
    50% {
      opacity: 0.35;
    }
  }
  .st-text {
    flex: 1;
    min-width: 0;
    display: grid;
    gap: 2px;
  }
  .st-text strong {
    font-size: var(--t-md);
    font-weight: 600;
  }
  .st-text small {
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
  .msg {
    color: var(--ink-2) !important;
  }
  .st-error .msg {
    color: var(--danger) !important;
  }
  .hw {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
  .recs {
    display: grid;
    gap: 8px;
  }
  .rec {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 11px 12px;
    border-radius: var(--r-md);
    box-shadow: inset 0 0 0 1px var(--rule);
  }
  .rec.active {
    box-shadow: inset 0 0 0 2px var(--accent);
    background: var(--accent-mist);
  }
  .rec-text {
    flex: 1;
    min-width: 0;
    display: grid;
    gap: 3px;
  }
  .rec-text strong {
    font-size: var(--t-md);
    font-weight: 600;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .rec-text small {
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
  .tag {
    font-size: 0.64rem;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    padding: 2px 6px;
    border-radius: 999px;
    color: var(--on-accent);
    background: var(--accent);
  }
  .tag.soft {
    color: var(--accent);
    background: var(--accent-mist);
  }
  .fit.tight {
    color: var(--warn-ink) !important;
  }
  .fit.no {
    color: var(--danger) !important;
  }
  .bar {
    height: 4px;
    margin-top: 4px;
    border-radius: 4px;
    background: var(--paper-3);
    overflow: hidden;
  }
  .bar span {
    display: block;
    height: 100%;
    background: var(--accent);
    transform-origin: left;
    transition: transform 0.25s linear;
  }
  .in-use {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: var(--t-xs);
    font-weight: 650;
    color: var(--accent);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .log {
    margin: 0;
    max-height: 220px;
    overflow: auto;
    padding: 10px;
    border-radius: var(--r-md);
    background: oklch(0.2 0.04 var(--tint-h));
    color: oklch(0.88 0.03 var(--tint-h));
    font-family: var(--font-mono);
    font-size: 0.7rem;
    line-height: 1.5;
    white-space: pre-wrap;
    user-select: text;
    -webkit-user-select: text;
  }
</style>
