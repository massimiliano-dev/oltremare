<script lang="ts">
  import Icon from "../Icon.svelte";
  import HotkeyField from "../agents/HotkeyField.svelte";
  import { api, errorText } from "../api";
  import { settings } from "../state/settings.svelte";
  import { ACCENTS } from "../state/theme.svelte";
  import { LAYOUTS } from "../state/layout.svelte";
  import { ui } from "../state/ui.svelte";

  const s = $derived(settings.s!);
  const custom = $derived(!ACCENTS.some((a) => a.hue === s.accentHue && a.chroma === s.accentChroma));

  async function setPalette(v: string) {
    if (v) {
      const why = await api.hotkeyCheck(v).catch(() => null);
      if (why && !why.includes("palette")) {
        ui.toast(why, "error");
        return;
      }
    }
    settings.save({ paletteHotkey: v });
  }
</script>

<section>
  <h3>Tema</h3>
  <div class="seg" role="radiogroup" aria-label="Tema">
    {#each [["system", "Sistema", "segue il computer"], ["light", "Chiaro", "carta"], ["dark", "Scuro", "inchiostro"]] as [id, label, sub] (id)}
      <button role="radio" aria-checked={s.theme === id} class:on={s.theme === id} onclick={() => settings.save({ theme: id as "system" | "light" | "dark" })}>
        <strong>{label}</strong><small>{sub}</small>
      </button>
    {/each}
  </div>
</section>

<section>
  <h3>Colore</h3>
  <div class="swatches" role="radiogroup" aria-label="Colore d'accento">
    {#each ACCENTS as a (a.id)}
      {@const on = a.hue === s.accentHue && a.chroma === s.accentChroma}
      <button role="radio" aria-checked={on} class:on onclick={() => settings.save({ accentHue: a.hue, accentChroma: a.chroma })} title={a.label}>
        <span class="sw" style="background: oklch(0.52 {a.chroma} {a.hue})"></span>
        <small>{a.label}</small>
      </button>
    {/each}
  </div>
  <label class="field">
    <span>Tonalità {custom ? "personalizzata" : ""}</span>
    <input type="range" min="0" max="359" step="1" value={s.accentHue} class="hue" oninput={(e) => settings.save({ accentHue: Number((e.target as HTMLInputElement).value) })} />
  </label>
  <label class="field">
    <span>Intensità</span>
    <input type="range" min="0.02" max="0.24" step="0.01" value={s.accentChroma} oninput={(e) => settings.save({ accentChroma: Number((e.target as HTMLInputElement).value) })} />
  </label>
</section>

<section>
  <h3>Testo e spazi</h3>
  <div class="seg" role="radiogroup" aria-label="Densità">
    {#each [["comfortable", "Comoda", "più aria"], ["compact", "Compatta", "più cose a schermo"]] as [id, label, sub] (id)}
      <button role="radio" aria-checked={s.density === id} class:on={s.density === id} onclick={() => settings.save({ density: id as "comfortable" | "compact" })}>
        <strong>{label}</strong><small>{sub}</small>
      </button>
    {/each}
  </div>
  <label class="field">
    <span>Dimensione del testo · {Math.round(s.fontScale * 100)}%</span>
    <input type="range" min="0.85" max="1.25" step="0.05" value={s.fontScale} oninput={(e) => settings.save({ fontScale: Number((e.target as HTMLInputElement).value) })} />
  </label>
</section>

<section>
  <h3>Disposizione</h3>
  <div class="layouts" role="radiogroup" aria-label="Disposizione dei pannelli">
    {#each LAYOUTS as l (l.id)}
      <button role="radio" aria-checked={s.layout === l.id} class:on={s.layout === l.id} onclick={() => settings.save({ layout: l.id })}>
        <span class="mini" aria-hidden="true">
          {#each l.cols as c, i (i)}<span class="c c-{c.kind}" style="flex: {c.flex}"></span>{/each}
        </span>
        <strong>{l.label}</strong>
        <small>{l.sub}</small>
      </button>
    {/each}
  </div>
  <small class="help">Trascina i bordi tra i pannelli per cambiarne la larghezza (doppio clic per tornare all'originale). ⌘B mostra o nasconde gli agenti, ⌘J il pannello laterale.</small>
</section>

<section>
  <h3>Finestre</h3>
  <div class="field">
    <span>Chiedi al volo da qualsiasi app</span>
    <HotkeyField value={s.paletteHotkey} onchange={setPalette} />
    <small class="help">Apre una piccola finestra per scrivere a un agente senza passare da Oltremare. Con «/» scegli l'agente.</small>
  </div>
  <label class="check">
    <input type="checkbox" checked={s.companion} onchange={(e) => api.companionSet((e.target as HTMLInputElement).checked).catch((err) => ui.toast(errorText(err), "error"))} />
    <span><strong>Compagno sullo schermo</strong><small>Oltremare sempre visibile, anche quando la finestra è chiusa.</small></span>
  </label>
  {#if s.companion}
    <div class="seg" role="radiogroup" aria-label="Stile del compagno">
      <button role="radio" aria-checked={s.companionStyle !== "floating"} class:on={s.companionStyle !== "floating"} onclick={() => settings.save({ companionStyle: "island" })}>
        <strong>Isola</strong><small>una pillola in alto, attorno alla notch, che si allarga quando ascolta, lavora o chiede</small>
      </button>
      <button role="radio" aria-checked={s.companionStyle === "floating"} class:on={s.companionStyle === "floating"} onclick={() => settings.save({ companionStyle: "floating" })}>
        <strong>Avatar</strong><small>il volto in una finestrella da trascinare: clic per parlare, doppio clic per aprire</small>
      </button>
    </div>
  {/if}
  <label class="check">
    <input type="checkbox" checked={s.autostart} onchange={(e) => api.autostartSet((e.target as HTMLInputElement).checked).catch((err) => ui.toast(errorText(err), "error"))} />
    <span><strong>Apri all'accensione</strong><small>Parte nella barra dei menu, senza finestra: pronto per voce, briefing e agenti programmati.</small></span>
  </label>
  <p class="soon"><Icon name="info" size={14} /> Chiudere la finestra non spegne Oltremare: resta nella barra dei menu. Per uscire usa il menu dell'icona.</p>
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
  h3 {
    margin: 0;
    font-size: var(--t-xs);
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--accent);
  }
  .swatches {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 8px;
  }
  .swatches button {
    display: grid;
    justify-items: center;
    gap: 5px;
    padding: 8px 4px;
    border-radius: var(--r-md);
    box-shadow: inset 0 0 0 1px var(--rule);
  }
  .swatches button.on {
    box-shadow: inset 0 0 0 2px var(--accent);
    background: var(--accent-mist);
  }
  .sw {
    width: 26px;
    height: 26px;
    border-radius: 50%;
  }
  .swatches small {
    font-size: var(--t-xs);
    color: var(--ink-2);
  }
  input[type="range"] {
    width: 100%;
    accent-color: var(--accent);
  }
  .hue {
    appearance: none;
    height: 10px;
    border-radius: 999px;
    background: linear-gradient(90deg, oklch(0.6 0.15 0), oklch(0.6 0.15 60), oklch(0.6 0.15 120), oklch(0.6 0.15 180), oklch(0.6 0.15 240), oklch(0.6 0.15 300), oklch(0.6 0.15 359));
  }
  .hue::-webkit-slider-thumb {
    appearance: none;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--paper);
    box-shadow: 0 0 0 2px var(--accent), 0 1px 4px oklch(0 0 0 / 0.2);
  }
  .layouts {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }
  .layouts button {
    display: grid;
    gap: 3px;
    padding: 10px;
    text-align: left;
    border-radius: var(--r-md);
    box-shadow: inset 0 0 0 1px var(--rule);
  }
  .layouts button.on {
    box-shadow: inset 0 0 0 2px var(--accent);
    background: var(--accent-mist);
  }
  .layouts strong {
    font-weight: 600;
    font-size: var(--t-sm);
  }
  .layouts small {
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
  .mini {
    display: flex;
    gap: 3px;
    height: 34px;
    margin-bottom: 4px;
  }
  .c {
    border-radius: 4px;
    background: var(--paper-2);
    box-shadow: inset 0 0 0 1px var(--rule);
  }
  .c-chat {
    background: var(--paper);
  }
  .c-side {
    background: var(--accent-wash);
    box-shadow: none;
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
  .soon {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    margin: 0;
    font-size: var(--t-xs);
    color: var(--ink-3);
    line-height: 1.45;
  }
</style>
