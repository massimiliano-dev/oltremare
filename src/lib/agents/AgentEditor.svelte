<script lang="ts">
  import { onMount, untrack } from "svelte";
  import Icon from "../Icon.svelte";
  import AgentDisc from "./AgentDisc.svelte";
  import { api, errorText, type Agent, type Provider } from "../api";
  import { agents, AGENT_ICONS, EFFORTS } from "../state/agents.svelte";
  import { settings, PROVIDERS, FALLBACK_MODELS, modelLabel } from "../state/settings.svelte";
  import { ui } from "../state/ui.svelte";
  import { fadeOut, sheet } from "../motion";
  import ToolPolicyTable from "./ToolPolicyTable.svelte";
  import TriggerList from "./TriggerList.svelte";
  import { local } from "../state/local.svelte";
  import Permissions from "../settings/Permissions.svelte";
  import { accounts } from "../state/accounts.svelte";

  let { agent }: { agent: Agent | "new" } = $props();

  // Si modifica una copia presa all'apertura: le modifiche esterne non toccano il form
  const initial = untrack(() => agent);
  const original: Agent = initial === "new" ? agents.blank() : structuredClone($state.snapshot(initial) as Agent);
  let draft = $state<Agent>(structuredClone(original));
  let models = $state<string[]>([]);
  let loadingModels = $state(false);
  let saving = $state(false);
  let confirmDiscard = $state(false);
  let confirmDelete = $state(false);
  let nameInput: HTMLInputElement | undefined = $state();

  const isNew = initial === "new";
  const dirty = $derived(JSON.stringify($state.snapshot(draft)) !== JSON.stringify(original));
  const nameTaken = $derived(agents.list.some((a) => a.id !== draft.id && a.name.trim().toLowerCase() === draft.name.trim().toLowerCase()));
  const issues = $derived([
    ...(!draft.name.trim() ? ["Dai un nome all'agente"] : []),
    ...(nameTaken ? ["Esiste già un agente con questo nome"] : []),
    ...(draft.provider === "local" && !local.installed.length ? ["Scarica prima un modello locale"] : []),
    ...(!draft.model.trim() && draft.provider !== "local" ? ["Scegli un modello"] : []),
  ]);
  const options = $derived(Array.from(new Set([draft.model, ...(models.length ? models : FALLBACK_MODELS[draft.provider])])).filter(Boolean));
  const tokensEstimate = $derived(Math.ceil(draft.systemPrompt.length / 4));

  async function loadModels() {
    models = [];
    if (draft.provider === "local" || draft.provider === "claude-code" || !settings.ready(draft.provider)) return;
    loadingModels = true;
    try {
      models = await api.models(draft.provider);
    } catch {
      models = [];
    } finally {
      loadingModels = false;
    }
  }

  const voicePhrases = $derived(
    ((draft.triggers as { kind: string; phrases?: string[] }[]).find((t) => t.kind === "voice")?.phrases ?? []).join(", "),
  );
  function setVoicePhrases(v: string) {
    const phrases = v.split(",").map((x) => x.trim()).filter(Boolean);
    const others = (draft.triggers as { kind: string }[]).filter((t) => t.kind !== "voice");
    draft.triggers = (phrases.length ? [...others, { kind: "voice", phrases }] : others) as Agent["triggers"];
  }

  async function addFolder() {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const r = await open({ directory: true, multiple: true, title: "Cartelle in cui l'agente può lavorare" });
      const list = r ? (Array.isArray(r) ? r : [r]) : [];
      draft.allowedPaths = [...new Set([...draft.allowedPaths, ...list])];
    } catch {}
  }

  function setProvider(p: Provider) {
    if (p === draft.provider) return;
    draft.provider = p;
    draft.model =
      p === "anthropic"
        ? (settings.s?.anthropicModel ?? "claude-opus-5-5")
        : p === "claude-code"
          ? (settings.s?.claudeCodeModel ?? "default")
          : p === "openai"
            ? (settings.s?.openaiModel ?? "gpt-5")
            : "";
    if (p === "local") draft.effort = "low";
    if (p === "openai") draft.effort = "";
    else if (!draft.effort) draft.effort = "medium";
    loadModels();
  }

  async function save() {
    if (issues.length || saving) return;
    saving = true;
    const saved = await agents.save($state.snapshot(draft) as Agent);
    saving = false;
    if (saved) {
      ui.editing = null;
      if (isNew) agents.select(saved.id);
      ui.toast(isNew ? `«${saved.name}» è pronto` : "Agente salvato");
    }
  }

  function close() {
    if (dirty && !confirmDiscard) {
      confirmDiscard = true;
      return;
    }
    ui.editing = null;
  }

  async function remove() {
    if (!confirmDelete) {
      confirmDelete = true;
      return;
    }
    ui.editing = null;
    await agents.remove(original);
  }

  function onKey(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "s") {
      e.preventDefault();
      save();
    } else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      close();
    }
  }

  onMount(() => {
    loadModels();
    if (isNew) nameInput?.focus();
  });
</script>

<svelte:window onkeydown={onKey} />

<div class="scrim" role="presentation" onclick={close} out:fadeOut></div>
<aside class="sheet" aria-label={isNew ? "Nuovo agente" : `Modifica ${original.name}`} out:sheet={{ x: 40, duration: 180 }}>
  <header>
    <AgentDisc icon={draft.icon} hue={draft.hue} size={44} />
    <div class="title">
      <span class="eyebrow">{isNew ? "Nuovo agente" : draft.system ? "Agente di sistema" : "Agente"}</span>
      <h2>{draft.name.trim() || "Senza nome"}</h2>
    </div>
    <button class="icon-btn" onclick={close} aria-label="Chiudi"><Icon name="x" /></button>
  </header>

  <div class="content">
    <section>
      <h3>Identità</h3>
      <label class="field">
        <span>Nome</span>
        <input class="input" bind:this={nameInput} bind:value={draft.name} placeholder="Es. Organizzatore, Ricercatore, Segretario…" maxlength="48" />
        {#if nameTaken}<small class="err">Esiste già un agente con questo nome</small>{/if}
      </label>
      <label class="field">
        <span>Descrizione</span>
        <input class="input" bind:value={draft.description} placeholder="Una riga: cosa fa questo agente" maxlength="120" />
      </label>
      <div class="field">
        <span>Icona e colore</span>
        <div class="icons" role="radiogroup" aria-label="Icona">
          {#each AGENT_ICONS as ic (ic)}
            <button role="radio" aria-checked={draft.icon === ic} class:on={draft.icon === ic} onclick={() => (draft.icon = ic)} aria-label={ic}>
              <Icon name={ic} size={16} />
            </button>
          {/each}
        </div>
        <input class="hue" type="range" min="0" max="359" step="1" bind:value={draft.hue} aria-label="Colore" style:--h={draft.hue} />
      </div>
    </section>

    <section>
      <h3>Modello</h3>
      <div class="seg" role="radiogroup" aria-label="Provider">
        {#each PROVIDERS as p (p.id)}
          <button role="radio" aria-checked={draft.provider === p.id} class:on={draft.provider === p.id} onclick={() => setProvider(p.id)}>
            <strong>{p.label}</strong><small>{p.sub}</small>
          </button>
        {/each}
      </div>
      {#if draft.provider === "local"}
        {#if !local.installed.length}
          <p class="note warn"><Icon name="cpu" size={15} /> Nessun modello locale scaricato. <button class="link" onclick={() => ui.openSettings("local")}>Scegline uno</button></p>
        {:else}
          <div class="two">
            <label class="field">
              <span>Modello locale</span>
              <select class="input" bind:value={draft.model}>
                <option value="">Predefinito ({local.label(settings.s?.localModel || local.installed[0]?.id)})</option>
                {#each local.installed as m (m.id)}<option value={m.id}>{m.label}</option>{/each}
              </select>
            </label>
            <label class="field">
              <span>Ragionamento</span>
              <select class="input" bind:value={draft.effort}>
                <option value="low">Spento · più rapido</option>
                <option value="medium">Attivo</option>
              </select>
            </label>
          </div>
          <small class="help">Gira sul tuo computer: gratis e privato. I modelli piccoli sono meno precisi con gli strumenti: per compiti delicati scegli un modello più grande.</small>
        {/if}
      {:else}
        {#if draft.provider === "claude-code"}
          {#if settings.claudeCode && !settings.claudeCode.loggedIn}
            <p class="note warn">
              <Icon name="key" size={15} /> {settings.claudeCode.installed ? "Claude Code non è collegato al tuo account." : "Claude Code non è installato su questo computer."}
              <button class="link" onclick={() => ui.openSettings("providers")}>Apri Provider</button>
            </p>
          {:else}
            <p class="note"><Icon name="user" size={15} /> Usa il tuo abbonamento Claude tramite Claude Code: nessuna chiave e nessun costo per richiesta, valgono i limiti del piano.</p>
          {/if}
        {:else if !settings.ready(draft.provider)}
          <p class="note warn">
            <Icon name="key" size={15} /> Nessuna chiave salvata per {PROVIDERS.find((p) => p.id === draft.provider)?.label}.
            <button class="link" onclick={() => ui.openSettings("providers")}>Apri Provider</button>
          </p>
        {/if}
        <div class="two">
          <label class="field">
            <span>Modello {#if loadingModels}<em class="loading">· carico…</em>{/if}</span>
            <select class="input" bind:value={draft.model}>
              {#each options as m (m)}<option value={m}>{draft.provider === "claude-code" ? modelLabel("claude-code", m) : m}</option>{/each}
            </select>
          </label>
          {#if draft.provider === "anthropic" || draft.provider === "claude-code"}
            <label class="field">
              <span>Impegno</span>
              <select class="input" bind:value={draft.effort}>
                {#each EFFORTS as e (e.id)}<option value={e.id}>{e.label}</option>{/each}
              </select>
            </label>
          {/if}
        </div>
        {#if draft.provider === "anthropic" || draft.provider === "claude-code"}
          <small class="help">Più impegno significa risposte più ragionate, ma più lente e costose. «Medio» va bene quasi sempre.</small>
        {/if}
      {/if}
    </section>

    <section>
      <h3>Strumenti</h3>
      <p class="help">Cosa può fare l'agente sul computer. «Chiedi» mostra una richiesta da approvare prima di agire.</p>
      {#if agents.catalog.length}
        <ToolPolicyTable bind:draft catalog={agents.catalog} />
        {#if agents.catalog.some((t) => t.category === "computer" && (draft.tools.includes(t.name) || draft.tools.includes("computer")))}
          <Permissions only={["screen", "accessibility"]} />
        {/if}
      {:else}
        <div class="skeleton" style="height:120px"></div>
      {/if}
    </section>

    <section>
      <h3>Avvio automatico</h3>
      <p class="help">Oltre a quando gli scrivi o gli parli, l'agente può partire da solo. Funziona finché Oltremare è aperto, anche solo nella barra dei menu.</p>
      <TriggerList bind:draft />
    </section>

    <section>
      <h3>Voce</h3>
      <label class="field">
        <span>Risposta a voce</span>
        <select class="input" bind:value={draft.voiceReply}>
          <option value="whenAsked">Solo quando gli parlo</option>
          <option value="always">Sempre</option>
          <option value="never">Mai</option>
        </select>
      </label>
      <label class="field">
        <span>Frasi che lo attivano a voce</span>
        <input class="input" value={voicePhrases} onchange={(e) => setVoicePhrases((e.target as HTMLInputElement).value)} placeholder="Es. riordina i download, organizza la cartella" />
        <small class="help">Separate da virgole. Se un comando vocale contiene una di queste frasi, risponde questo agente.</small>
      </label>
    </section>

    <section>
      <h3>Limiti</h3>
      <div class="field">
        <span>Cartelle consentite</span>
        <ul class="folders">
          {#each draft.allowedPaths as p (p)}
            <li><Icon name="folder" size={14} /><span>{p}</span><button onclick={() => (draft.allowedPaths = draft.allowedPaths.filter((x) => x !== p))} aria-label="Togli"><Icon name="x" size={12} /></button></li>
          {:else}
            <li class="default"><Icon name="folder" size={14} /><span>Scrivania, Documenti e Download (predefinite)</span></li>
          {/each}
        </ul>
        <button class="btn btn-line add" onclick={addFolder}><Icon name="plus" size={14} /> Aggiungi cartella</button>
        <small class="help">File e comandi restano dentro queste cartelle. Chiavi, portachiavi e cartelle di sistema sono sempre escluse.</small>
      </div>
      {#if accounts.list.length > 1}
        <div class="field">
          <span>Account che può usare</span>
          {#each accounts.list as acc (acc.id)}
            <label class="check">
              <input
                type="checkbox"
                checked={!draft.mailAccounts.length || draft.mailAccounts.includes(acc.id)}
                onchange={(e) => {
                  const all = draft.mailAccounts.length ? draft.mailAccounts : accounts.list.map((x) => x.id);
                  const next = (e.target as HTMLInputElement).checked ? [...new Set([...all, acc.id])] : all.filter((x) => x !== acc.id);
                  draft.mailAccounts = next.length === accounts.list.length ? [] : next;
                }}
              />
              <span>{acc.email}</span>
            </label>
          {/each}
        </div>
      {/if}
      <label class="field">
        <span>Siti consentiti nel browser</span>
        <input class="input" value={draft.browserDomains.join(", ")} onchange={(e) => (draft.browserDomains = (e.target as HTMLInputElement).value.split(",").map((x) => x.trim().toLowerCase()).filter(Boolean))} placeholder="Tutti (oppure es. comune.milano.it, amazon.it)" />
        <small class="help">Vale anche per i sottodomini. Vuoto: l'agente può aprire qualsiasi sito.</small>
      </label>
      <div class="two">
        <label class="field">
          <span>Tempo per approvare</span>
          <select class="input" bind:value={draft.policy.approvalTimeoutS}>
            {#each [[60, "1 minuto"], [180, "3 minuti"], [300, "5 minuti"], [600, "10 minuti"], [1800, "30 minuti"]] as [v, l] (v)}<option value={v}>{l}</option>{/each}
          </select>
        </label>
        <label class="field">
          <span>Passi massimi</span>
          <input class="input" type="number" min="1" max="200" bind:value={draft.maxTurns} />
        </label>
      </div>
      <label class="check">
        <input type="checkbox" bind:checked={draft.policy.allowSession} />
        <span>Permetti «Sempre in questa esecuzione» nelle richieste</span>
      </label>
    </section>

    <section>
      <h3>Istruzioni</h3>
      <label class="field">
        <span>Chi è e come lavora</span>
        <textarea
          class="input prompt"
          bind:value={draft.systemPrompt}
          rows="9"
          placeholder="Es. Sei il mio organizzatore. Quando ti chiedo di pianificare, proponi una scaletta con orari realistici e chiedimi conferma prima di decidere per me."
        ></textarea>
      </label>
      <small class="help">Circa {tokensEstimate} token. Oltremare aggiunge da solo lingua, data e regole di sicurezza.</small>
    </section>
  </div>

  <footer>
    {#if confirmDiscard}
      <span class="ask">Modifiche non salvate.</span>
      <button class="btn btn-quiet" onclick={() => (confirmDiscard = false)}>Continua a modificare</button>
      <button class="btn btn-danger" onclick={() => (ui.editing = null)}>Scarta</button>
    {:else if confirmDelete}
      <span class="ask">Eliminare «{original.name}» e le sue conversazioni?</span>
      <button class="btn btn-quiet" onclick={() => (confirmDelete = false)}>Annulla</button>
      <button class="btn btn-danger" onclick={remove}>Elimina</button>
    {:else}
      {#if !isNew && !draft.system}
        <button class="btn btn-danger" onclick={remove}><Icon name="trash" size={15} /> Elimina</button>
      {/if}
      {#if issues.length && draft.name.trim()}<span class="issues">{issues[0]}</span>{/if}
      <span class="spacer"></span>
      <button class="btn btn-quiet" onclick={close}>Annulla</button>
      <button class="btn btn-primary" onclick={save} disabled={!!issues.length || saving || (!dirty && !isNew)}>{isNew ? "Crea agente" : "Salva"} <kbd>⌘S</kbd></button>
    {/if}
  </footer>
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
  .sheet {
    position: absolute;
    z-index: 61;
    top: 0;
    right: 0;
    bottom: 0;
    width: min(620px, 100vw);
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
    gap: 14px;
    padding: 40px 26px 16px;
    border-bottom: 1px solid var(--rule-soft);
  }
  .title {
    flex: 1;
    min-width: 0;
  }
  h2 {
    margin: 2px 0 0;
    font-family: var(--font-serif);
    font-weight: 400;
    font-size: 1.9rem;
    line-height: 1.05;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .content {
    flex: 1;
    overflow-y: auto;
    padding: 0 26px;
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
  .icons {
    display: grid;
    grid-template-columns: repeat(12, 1fr);
    gap: 4px;
  }
  .icons button {
    display: grid;
    place-items: center;
    aspect-ratio: 1;
    border-radius: var(--r-sm);
    color: var(--ink-2);
  }
  .icons button:hover {
    background: var(--accent-mist);
    color: var(--accent);
  }
  .icons button.on {
    color: var(--on-accent);
    background: var(--accent);
  }
  .hue {
    width: 100%;
    height: 14px;
    margin-top: 6px;
    appearance: none;
    -webkit-appearance: none;
    border-radius: 999px;
    background: linear-gradient(
      90deg,
      oklch(0.55 0.17 0),
      oklch(0.55 0.17 60),
      oklch(0.55 0.17 120),
      oklch(0.55 0.17 180),
      oklch(0.55 0.17 240),
      oklch(0.55 0.17 300),
      oklch(0.55 0.17 359)
    );
  }
  .hue::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: oklch(0.55 0.17 var(--h));
    border: 3px solid var(--paper);
    box-shadow: 0 0 0 1px var(--rule), 0 2px 6px oklch(0 0 0 / 0.25);
  }
  .two {
    display: grid;
    grid-template-columns: 1.4fr 1fr;
    gap: 12px;
  }
  .loading {
    font-style: normal;
    text-transform: none;
    letter-spacing: 0;
    font-weight: 500;
  }
  .note {
    display: flex;
    align-items: flex-start;
    flex-wrap: wrap;
    gap: 8px;
    margin: 0;
    padding: 10px 12px;
    border-radius: var(--r-md);
    font-size: var(--t-sm);
    line-height: 1.45;
    color: var(--ink-2);
    background: var(--paper-2);
  }
  .note :global(svg) {
    flex: none;
    margin-top: 1px;
    color: var(--accent);
  }
  .note.warn :global(svg) {
    color: var(--warn-ink);
  }
  .folders {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 4px;
  }
  .folders li {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 8px 7px 10px;
    border-radius: var(--r-sm);
    background: var(--paper-2);
    font-size: var(--t-sm);
    color: var(--ink-2);
  }
  .folders li span {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: 0.78rem;
  }
  .folders li.default span {
    font-family: var(--font-sans);
    font-size: var(--t-sm);
    color: var(--ink-3);
  }
  .folders li :global(svg) {
    color: var(--accent);
    flex: none;
  }
  .folders button {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 6px;
    color: var(--ink-3);
  }
  .folders button:hover {
    color: var(--danger);
  }
  .add {
    justify-self: start;
    margin-top: 4px;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--t-sm);
  }
  .check input {
    accent-color: var(--accent);
  }
  .prompt {
    min-height: 180px;
    font-size: var(--t-md);
  }
  footer {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 14px 26px;
    border-top: 1px solid var(--rule-soft);
    background: var(--paper-2);
  }
  .spacer {
    flex: 1;
  }
  .ask {
    flex: 1;
    font-size: var(--t-sm);
    font-weight: 600;
  }
  .issues {
    font-size: var(--t-xs);
    color: var(--warn-ink);
  }
  footer kbd {
    background: oklch(1 0 0 / 0.18);
    color: inherit;
    box-shadow: none;
  }
</style>
