<script lang="ts">
  import Icon from "../Icon.svelte";
  import Avatar from "../avatar/Avatar.svelte";
  import { api, errorText, type AvatarState, type Provider, type ProviderTest } from "../api";
  import { settings, FALLBACK_MODELS, modelLabel } from "../state/settings.svelte";
  import { onMount } from "svelte";
  import LocalAiTab from "./LocalAiTab.svelte";
  import VoiceTab from "./VoiceTab.svelte";
  import Permissions from "./Permissions.svelte";
  import AccountsTab from "./AccountsTab.svelte";
  import NotificationsTab from "./NotificationsTab.svelte";
  import ToolsTab from "./ToolsTab.svelte";
  import AppearanceTab from "./AppearanceTab.svelte";
  import { ui, type SettingsTab } from "../state/ui.svelte";
  import { STATE_LABEL } from "../state/avatar.svelte";
  import { fadeOut, sheet } from "../motion";

  const TABS: { id: SettingsTab; label: string; icon: string }[] = [
    { id: "general", label: "Generale", icon: "settings" },
    { id: "appearance", label: "Aspetto", icon: "palette" },
    { id: "providers", label: "Provider", icon: "key" },
    { id: "local", label: "AI locale", icon: "cpu" },
    { id: "voice", label: "Voce", icon: "mic" },
    { id: "accounts", label: "Account", icon: "mail" },
    { id: "notifications", label: "Notifiche", icon: "bell" },
    { id: "tools", label: "Strumenti esterni", icon: "layers" },
    { id: "assistant", label: "Assistente", icon: "face" },
  ];

  const s = $derived(settings.s!);
  let keys = $state<Record<Provider, string>>({ anthropic: "", "claude-code": "", openai: "", local: "" });
  let tests = $state<Partial<Record<Provider, ProviderTest | "…">>>({});
  let models = $state<Partial<Record<Provider, string[]>>>({});
  let preview = $state<AvatarState>("idle");

  async function saveKey(p: Provider) {
    try {
      await api.setKey(p, keys[p]);
      keys[p] = "";
      await test(p);
    } catch (e) {
      ui.toast(errorText(e), "error");
    }
  }

  async function removeKey(p: Provider) {
    await api.setKey(p, "").catch(() => {});
    tests[p] = undefined;
    models[p] = undefined;
  }

  async function test(p: Provider) {
    tests[p] = "…";
    const r = await api.testProvider(p).catch((e) => ({ ok: false, ms: 0, models: 0, error: errorText(e) }) as ProviderTest);
    tests[p] = r;
    if (r.ok) models[p] = await api.models(p).catch(() => []);
  }

  function opts(p: Provider, current: string) {
    return Array.from(new Set([current, ...(models[p]?.length ? models[p]! : FALLBACK_MODELS[p])])).filter(Boolean);
  }

  onMount(() => {
    if (ui.settingsTab === "providers") settings.checkClaudeCode();
  });

  const PLAN: Record<string, string> = { max: "Max", pro: "Pro", team: "Team", enterprise: "Enterprise" };

  const PREVIEW_STATES: AvatarState[] = ["idle", "listening", "thinking", "speaking", "working", "asking", "alert", "error", "sleep"];
</script>

<div class="scrim" role="presentation" onclick={() => (ui.showSettings = false)} out:fadeOut></div>
<aside class="panel" aria-label="Impostazioni" out:sheet={{ x: 40, duration: 180 }}>
  <header>
    <h2>Impostazioni</h2>
    <button class="icon-btn" onclick={() => (ui.showSettings = false)} aria-label="Chiudi"><Icon name="x" /></button>
  </header>
  <div class="tabs" role="tablist">
    {#each TABS as t (t.id)}
      <button role="tab" aria-selected={ui.settingsTab === t.id} class:on={ui.settingsTab === t.id} onclick={() => (ui.settingsTab = t.id)}>
        <Icon name={t.icon} size={15} /> {t.label}
      </button>
    {/each}
  </div>

  <div class="content">
    {#if ui.settingsTab === "general"}
      <section>
        <label class="field">
          <span>Come ti chiami</span>
          <input class="input" value={s.userName} onchange={(e) => settings.save({ userName: (e.target as HTMLInputElement).value })} placeholder="Il nome con cui ti saluta" />
        </label>
        <label class="field">
          <span>Lingua delle risposte</span>
          <select class="input" value={s.language} onchange={(e) => settings.save({ language: (e.target as HTMLSelectElement).value })}>
            <option value="italiano">Italiano</option>
            <option value="inglese">English</option>
            <option value="spagnolo">Español</option>
            <option value="francese">Français</option>
            <option value="tedesco">Deutsch</option>
          </select>
        </label>
      </section>
      <section>
        <label class="check">
          <input type="checkbox" checked={s.sendWithEnter} onchange={(e) => settings.save({ sendWithEnter: (e.target as HTMLInputElement).checked })} />
          <span><strong>Invia con Invio</strong><small>Altrimenti si invia con ⌘↵ e Invio va a capo</small></span>
        </label>
        <label class="check">
          <input type="checkbox" checked={s.showThinking} onchange={(e) => settings.save({ showThinking: (e.target as HTMLInputElement).checked })} />
          <span><strong>Mostra il ragionamento</strong><small>Un riassunto di come l'agente arriva alla risposta, richiudibile</small></span>
        </label>
      </section>
      <section>
        <h3>Permessi di sistema</h3>
        <Permissions />
      </section>
      <section>
        <h3>Browser degli agenti</h3>
        {#await api.browserStatus() then b}
          <p class="soon"><Icon name="globe" size={15} /> {b.installed ? `Usa ${b.installed.split(/[\/]/).pop()?.replace(".exe", "")} con un profilo separato dal tuo: cookie e accessi degli agenti restano lì.` : "Installa Chrome o Edge per far navigare gli agenti."}</p>
        {/await}
        <div class="row-btns">
          <button class="btn btn-line" onclick={() => api.browserClose()}>Chiudi il browser</button>
          <button class="btn btn-quiet btn-danger" onclick={async () => { await api.browserResetProfile(); ui.toast("Profilo del browser azzerato"); }}>Cancella cookie e accessi</button>
        </div>
      </section>
    {:else if ui.settingsTab === "providers"}
      {@const cc = settings.claudeCode}
      <section>
        <div class="prov-head">
          <h3>Claude con il tuo abbonamento</h3>
          <button class="btn btn-line" onclick={() => settings.checkClaudeCode()} disabled={settings.checkingClaudeCode}>{settings.checkingClaudeCode ? "Controllo…" : "Ricontrolla"}</button>
        </div>
        {#if !cc}
          <p class="test"><span class="skeleton" style="width:60%;height:14px"></span></p>
        {:else if cc.loggedIn}
          <div class="key-ok">
            <Icon name="check" size={15} />
            <span>Collegato come <strong>{cc.email}</strong>{cc.subscription ? ` · piano ${PLAN[cc.subscription] ?? cc.subscription}` : ""}{cc.version ? ` · Claude Code ${cc.version}` : ""}</span>
          </div>
        {:else if cc.installed}
          <p class="test bad"><Icon name="alert" size={14} /> Claude Code è installato ma non è collegato a un account. Apri il Terminale, esegui <code>claude</code> e usa <code>/login</code>, poi premi «Ricontrolla».</p>
        {:else}
          <p class="test bad"><Icon name="alert" size={14} /> Claude Code non è installato. Scaricalo da claude.com/download, accedi con il tuo account e premi «Ricontrolla».</p>
        {/if}
        <small class="help">
          Oltremare avvia Claude Code sul tuo computer con il tuo accesso Pro o Max: nessuna chiave API e nessun costo per richiesta, ma valgono i limiti di utilizzo del piano. È pensato per uso personale su questo computer.
        </small>
        <label class="field">
          <span>Modello predefinito</span>
          <select class="input" value={s.claudeCodeModel} onchange={(e) => settings.save({ claudeCodeModel: (e.target as HTMLSelectElement).value })}>
            {#each FALLBACK_MODELS["claude-code"] as m (m)}<option value={m}>{modelLabel("claude-code", m)}</option>{/each}
          </select>
        </label>
        <label class="field">
          <span>Percorso di Claude Code</span>
          <input
            class="input"
            value={s.claudeCodePath}
            placeholder={cc?.path ?? "Trovato da solo"}
            spellcheck="false"
            onchange={async (e) => {
              await settings.save({ claudeCodePath: (e.target as HTMLInputElement).value.trim() });
              settings.checkClaudeCode();
            }}
          />
          <small class="help">Lascialo vuoto: Oltremare lo cerca da solo.</small>
        </label>
      </section>
      {#each [{ p: "anthropic" as Provider, name: "Claude", where: "console.anthropic.com → API Keys", ph: "sk-ant-…" }, { p: "openai" as Provider, name: "GPT", where: "platform.openai.com → API keys", ph: "sk-…" }] as row (row.p)}
        {@const has = settings.state?.keys[row.p as "anthropic" | "openai"]}
        {@const t = tests[row.p]}
        <section>
          <div class="prov-head">
            <h3>{row.name}</h3>
            {#if has}
              <button class="btn btn-line" onclick={() => test(row.p)} disabled={t === "…"}>{t === "…" ? "Verifico…" : "Verifica"}</button>
            {/if}
          </div>
          {#if has}
            <div class="key-ok">
              <Icon name="check" size={15} />
              <span>Chiave salvata nel portachiavi di sistema</span>
              <button class="link" onclick={() => removeKey(row.p)}>Rimuovi</button>
            </div>
          {/if}
          {#if t && t !== "…"}
            <p class="test" class:bad={!t.ok}>
              {#if t.ok}<Icon name="check" size={14} /> Funziona · risposta in {t.ms} ms · {t.models} modelli disponibili{:else}<Icon name="alert" size={14} /> {t.error}{/if}
            </p>
          {/if}
          <div class="key-row">
            <input class="input" type="password" bind:value={keys[row.p]} placeholder={has ? "Sostituisci la chiave…" : row.ph} spellcheck="false" autocomplete="off" />
            <button class="btn btn-primary" disabled={!keys[row.p].trim()} onclick={() => saveKey(row.p)}>Salva</button>
          </div>
          <small class="help">{row.where}. La chiave resta sul tuo computer: le richieste partono direttamente dall'app, mai dalla finestra.</small>
          <label class="field">
            <span>Modello predefinito per i nuovi agenti</span>
            {#if row.p === "anthropic"}
              <select class="input" value={s.anthropicModel} onfocus={() => has && !models.anthropic && test("anthropic")} onchange={(e) => settings.save({ anthropicModel: (e.target as HTMLSelectElement).value })}>
                {#each opts("anthropic", s.anthropicModel) as m (m)}<option value={m}>{m}</option>{/each}
              </select>
            {:else}
              <select class="input" value={s.openaiModel} onfocus={() => has && !models.openai && test("openai")} onchange={(e) => settings.save({ openaiModel: (e.target as HTMLSelectElement).value })}>
                {#each opts("openai", s.openaiModel) as m (m)}<option value={m}>{m}</option>{/each}
              </select>
            {/if}
          </label>
          {#if row.p === "openai"}
            <label class="field">
              <span>Endpoint</span>
              <input class="input" value={s.openaiBaseUrl} onchange={(e) => settings.save({ openaiBaseUrl: (e.target as HTMLInputElement).value })} spellcheck="false" />
              <small class="help">Cambialo per Azure OpenAI o altri servizi compatibili.</small>
            </label>
          {/if}
        </section>
      {/each}
      <section>
        <h3>Locale</h3>
        <p class="soon"><Icon name="cpu" size={15} /> I modelli che girano sul tuo computer si gestiscono nella scheda <button class="link" onclick={() => (ui.settingsTab = "local")}>AI locale</button>.</p>
      </section>
    {:else if ui.settingsTab === "local"}
      <LocalAiTab />
    {:else if ui.settingsTab === "voice"}
      <VoiceTab />
    {:else if ui.settingsTab === "accounts"}
      <AccountsTab />
    {:else if ui.settingsTab === "notifications"}
      <NotificationsTab />
    {:else if ui.settingsTab === "tools"}
      <ToolsTab />
    {:else if ui.settingsTab === "appearance"}
      <AppearanceTab />
    {:else}
      <section>
        <div class="field">
          <span>Volto</span>
          <div class="faces">
            {#each [["character", "Personaggio", "un piccolo robot con le espressioni"], ["orb", "Orb", "una sfera di luce che respira"]] as [id, label, sub] (id)}
              <button class="face" class:on={s.avatarFace === id} onclick={() => settings.save({ avatarFace: id as "character" | "orb" })}>
                <Avatar size={96} face={id as "character" | "orb"} state={preview} level={preview === "speaking" ? 0.6 : preview === "listening" ? 0.4 : 0} track={false} />
                <strong>{label}</strong><small>{sub}</small>
              </button>
            {/each}
          </div>
        </div>
        <div class="field">
          <span>Prova le espressioni</span>
          <div class="states">
            {#each PREVIEW_STATES as st (st)}
              <button class:on={preview === st} onclick={() => (preview = st)}>{STATE_LABEL[st]}</button>
            {/each}
          </div>
        </div>
        <small class="help">L'avatar mostra cosa sta facendo Oltremare. Con la voce (in arrivo) muoverà la bocca mentre parla e reagirà quando lo chiami.</small>
      </section>
    {/if}
  </div>
</aside>

<style>
  .scrim {
    position: absolute;
    inset: 0;
    z-index: 60;
    background: var(--scrim);
    animation: fade 0.2s;
  }
  @keyframes fade {
    from {
      opacity: 0;
    }
  }
  .panel {
    position: absolute;
    z-index: 61;
    top: 0;
    right: 0;
    bottom: 0;
    width: min(500px, 100vw);
    display: flex;
    flex-direction: column;
    background: var(--paper);
    box-shadow: var(--shadow-sheet);
    animation: slide 0.32s var(--ease-out);
  }
  @keyframes slide {
    from {
      transform: translateX(40px);
      opacity: 0;
    }
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 40px 26px 10px;
  }
  h2 {
    margin: 0;
    font-family: var(--font-serif);
    font-weight: 400;
    font-size: 2rem;
  }
  .tabs {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    padding: 0 22px;
    border-bottom: 1px solid var(--rule-soft);
  }
  .tabs button {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    height: 38px;
    padding: 0 10px;
    font-size: var(--t-sm);
    font-weight: 600;
    color: var(--ink-3);
    border-bottom: 2px solid transparent;
    margin-bottom: -1px;
  }
  .tabs button:hover {
    color: var(--ink);
  }
  .tabs button.on {
    color: var(--accent);
    border-bottom-color: var(--accent);
  }
  .content {
    flex: 1;
    overflow-y: auto;
    padding: 0 26px 30px;
  }
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
  .prov-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .key-ok {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--t-sm);
    color: var(--ok);
  }
  .key-ok span {
    flex: 1;
    color: var(--ink-2);
  }
  .key-row {
    display: flex;
    gap: 8px;
  }
  .key-row .btn {
    height: 38px;
  }
  .test {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    margin: 0;
    font-size: var(--t-sm);
    color: var(--ok);
  }
  .test.bad {
    color: var(--danger);
  }
  .test :global(svg) {
    flex: none;
    margin-top: 2px;
  }
  .test code {
    font-family: var(--font-mono);
    font-size: 0.85em;
    padding: 0.05em 0.3em;
    border-radius: 4px;
    background: var(--paper-3);
  }
  .key-ok strong {
    font-weight: 600;
    color: var(--ink);
  }
  .soon {
    display: flex;
    gap: 8px;
    margin: 0;
    font-size: var(--t-sm);
    line-height: 1.5;
    color: var(--ink-2);
  }
  .soon :global(svg) {
    flex: none;
    margin-top: 2px;
    color: var(--accent);
  }
  .row-btns {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
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
    font-size: var(--t-md);
    font-weight: 600;
  }
  .check small {
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
  .faces {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }
  .face {
    display: grid;
    justify-items: center;
    gap: 2px;
    padding: 14px 10px 12px;
    border-radius: var(--r-lg);
    box-shadow: inset 0 0 0 1px var(--rule);
    background: radial-gradient(100% 80% at 50% 35%, var(--accent-mist), transparent 70%);
  }
  .face.on {
    box-shadow: inset 0 0 0 2px var(--accent);
  }
  .face strong {
    margin-top: 6px;
    font-family: var(--font-serif);
    font-weight: 400;
    font-size: 1.25rem;
  }
  .face small {
    font-size: var(--t-xs);
    color: var(--ink-3);
    text-align: center;
  }
  .states {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .states button {
    height: 28px;
    padding: 0 11px;
    border-radius: 999px;
    font-size: var(--t-xs);
    font-weight: 600;
    color: var(--ink-2);
    box-shadow: inset 0 0 0 1px var(--rule);
  }
  .states button:hover {
    color: var(--accent);
  }
  .states button.on {
    color: var(--on-accent);
    background: var(--accent);
    box-shadow: none;
  }
</style>
