<script lang="ts">
  import { onMount } from "svelte";
  import Icon from "../Icon.svelte";
  import HotkeyField from "../agents/HotkeyField.svelte";
  import { api, errorText } from "../api";
  import { settings } from "../state/settings.svelte";
  import { local } from "../state/local.svelte";
  import { voice } from "../state/voice.svelte";
  import { agents } from "../state/agents.svelte";
  import { ui } from "../state/ui.svelte";
  import { fileSize } from "../format";

  const s = $derived(settings.s!);
  let devices = $state<string[]>([]);
  const vm = $derived(local.catalog.filter((m) => m.kind !== "llm"));
  const need = $derived(["silero-vad", s.voiceStt, ...(s.voiceTts === "sistema" ? [] : [s.voiceTts])]);
  const missing = $derived(vm.filter((m) => need.includes(m.id) && !m.installed));

  onMount(async () => {
    devices = await api.voiceDevices().catch(() => []);
    await Promise.all([local.refresh(), voice.refresh()]);
  });

  async function setListen(on: boolean) {
    try {
      await api.voiceListenSet(on);
      await voice.refresh();
    } catch (e) {
      ui.toast(errorText(e), "error");
    }
  }

  async function downloadAll() {
    for (const m of missing) await local.download(m.id);
  }

  async function hotkey(v: string) {
    try {
      await api.voiceHotkeySet(v);
    } catch (e) {
      ui.toast(errorText(e), "error");
    }
  }

  const SAMPLE = "Ciao! Sono Oltremare. Dimmi cosa posso fare per te.";
</script>

<section>
  {#if missing.length}
    <div class="setup">
      <Icon name="download" size={18} />
      <div>
        <strong>Prepara la voce</strong>
        <small>Servono {missing.map((m) => m.label.toLowerCase()).join(", ")} · {fileSize(missing.reduce((a, m) => a + m.sizeBytes, 0))}. Tutto resta sul tuo computer.</small>
      </div>
      <button class="btn btn-primary" onclick={downloadAll} disabled={missing.every((m) => local.downloads[m.id])}>Scarica</button>
    </div>
    {#each missing as m (m.id)}
      {@const p = local.downloads[m.id]}
      {#if p && !p.done}
        <div class="dl"><span>{m.label}</span><div class="bar"><span style:transform="scaleX({p.total ? p.received / p.total : 0})"></span></div><small>{fileSize(p.received)}</small></div>
      {/if}
    {/each}
  {/if}

  <label class="check big">
    <input type="checkbox" checked={s.voiceListen} disabled={!voice.ready} onchange={(e) => setListen((e.target as HTMLInputElement).checked)} />
    <span><strong>Ascolta «Ehi Oltremare»</strong><small>Il microfono resta acceso e l'audio viene elaborato solo sul tuo computer. Puoi dire anche il comando subito dopo: «Ehi Oltremare, che impegni ho domani?»</small></span>
  </label>
  <div class="field">
    <span>Tieni premuto per parlare</span>
    <HotkeyField value={s.voiceHotkey} onchange={hotkey} />
    <small class="help">Funziona anche con Oltremare in secondo piano. In alternativa, il pulsante del microfono nella chat.</small>
  </div>
</section>

<section>
  <h3>Riconoscimento</h3>
  <div class="two">
    <label class="field">
      <span>Microfono</span>
      <select class="input" value={s.voiceDevice} onchange={(e) => settings.save({ voiceDevice: (e.target as HTMLSelectElement).value })}>
        <option value="">Predefinito di sistema</option>
        {#each devices as d (d)}<option value={d}>{d}</option>{/each}
      </select>
    </label>
    <label class="field">
      <span>Modello</span>
      <select class="input" value={s.voiceStt} onchange={(e) => settings.save({ voiceStt: (e.target as HTMLSelectElement).value })}>
        {#each vm.filter((m) => m.kind === "stt") as m (m.id)}<option value={m.id}>{m.label}{m.installed ? "" : ` · da scaricare (${fileSize(m.sizeBytes)})`}</option>{/each}
      </select>
    </label>
  </div>
  <label class="field">
    <span>Chi risponde ai comandi vocali</span>
    <select class="input" value={s.voiceAgent} onchange={(e) => settings.save({ voiceAgent: (e.target as HTMLSelectElement).value })}>
      {#each agents.list as a (a.id)}<option value={a.id}>{a.name}</option>{/each}
    </select>
    <small class="help">Gli agenti con una frase di attivazione a voce rispondono quando la pronunci.</small>
  </label>
</section>

<section>
  <h3>Voce delle risposte</h3>
  <div class="voices">
    {#each [...vm.filter((m) => m.kind === "tts"), { id: "sistema", label: "Di sistema", note: "La voce del computer, sempre disponibile", installed: true, sizeBytes: 0 }] as v (v.id)}
      <div class="voice" class:on={s.voiceTts === v.id}>
        <button class="pick" onclick={() => settings.save({ voiceTts: v.id })}>
          <strong>{v.label}</strong><small>{v.note}</small>
        </button>
        {#if v.installed}
          <button class="icon-btn" title="Ascolta" aria-label="Ascolta {v.label}" onclick={async () => { await settings.save({ voiceTts: v.id }); api.voiceSpeak(SAMPLE); }}><Icon name="arrowUp" size={15} /></button>
        {:else}
          <button class="btn btn-line" onclick={() => local.download(v.id)} disabled={!!local.downloads[v.id]}><Icon name="download" size={13} /> {fileSize(v.sizeBytes)}</button>
        {/if}
      </div>
    {/each}
  </div>
  <label class="field">
    <span>Velocità · {s.voiceSpeed.toFixed(1).replace(".", ",")}×</span>
    <input type="range" min="0.7" max="1.4" step="0.1" value={s.voiceSpeed} onchange={(e) => settings.save({ voiceSpeed: Number((e.target as HTMLInputElement).value) })} />
  </label>
  <small class="help">Ogni agente decide se rispondere a voce (sempre, solo se gli parli, mai) nelle sue impostazioni. Di' «basta» per interrompere.</small>
</section>

<style>
  section {
    display: grid;
    gap: 14px;
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
  .setup {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 14px;
    border-radius: var(--r-md);
    background: var(--accent-mist);
    box-shadow: inset 0 0 0 1px var(--accent-wash);
    color: var(--accent);
  }
  .setup div {
    flex: 1;
    display: grid;
    gap: 2px;
  }
  .setup strong {
    color: var(--ink);
    font-weight: 650;
  }
  .setup small {
    font-size: var(--t-xs);
    color: var(--ink-2);
  }
  .dl {
    display: grid;
    grid-template-columns: 140px 1fr auto;
    align-items: center;
    gap: 10px;
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
  .bar {
    height: 4px;
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
  .check {
    display: flex;
    align-items: flex-start;
    gap: 10px;
  }
  .check input {
    margin-top: 3px;
    accent-color: var(--accent);
  }
  .check span {
    display: grid;
    gap: 2px;
  }
  .check strong {
    font-weight: 600;
  }
  .check small {
    font-size: var(--t-xs);
    color: var(--ink-3);
    line-height: 1.45;
  }
  .two {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }
  .voices {
    display: grid;
    gap: 6px;
  }
  .voice {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 6px 4px 4px;
    border-radius: var(--r-md);
    box-shadow: inset 0 0 0 1px var(--rule);
  }
  .voice.on {
    box-shadow: inset 0 0 0 2px var(--accent);
    background: var(--accent-mist);
  }
  .pick {
    flex: 1;
    display: grid;
    gap: 1px;
    padding: 6px 8px;
    text-align: left;
  }
  .pick strong {
    font-family: var(--font-serif);
    font-weight: 400;
    font-size: 1.15rem;
  }
  .pick small {
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
  input[type="range"] {
    accent-color: var(--accent);
  }
</style>
