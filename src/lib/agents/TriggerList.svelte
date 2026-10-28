<script lang="ts">
  // Quando parte l'agente da solo: a orario, da una cartella, da un'email, da una notifica, con una scorciatoia.
  import Icon from "../Icon.svelte";
  import HotkeyField from "./HotkeyField.svelte";
  import { api, type Agent, type Trigger } from "../api";
  import { accounts } from "../state/accounts.svelte";
  import { listDate } from "../format";

  let { draft = $bindable() }: { draft: Agent } = $props();
  type K = Exclude<Trigger["kind"], "voice">;

  const KINDS: { kind: K; label: string; icon: string }[] = [
    { kind: "cron", label: "A un orario", icon: "clock" },
    { kind: "folder", label: "Quando arrivano file", icon: "folder" },
    { kind: "mail", label: "Quando arriva un'email", icon: "mail" },
    { kind: "notification", label: "Quando arriva una notifica", icon: "bell" },
    { kind: "hotkey", label: "Con una scorciatoia", icon: "command" },
  ];
  const CRON_PRESETS: [string, string][] = [
    ["0 9 * * 1-5", "Giorni feriali alle 9"],
    ["0 8 * * *", "Ogni giorno alle 8"],
    ["0 * * * *", "Ogni ora"],
    ["*/15 * * * *", "Ogni 15 minuti"],
    ["0 18 * * 5", "Venerdì alle 18"],
    ["0 9 1 * *", "Il primo del mese alle 9"],
  ];

  let adding = $state(false);
  let previews = $state<Record<number, string>>({});
  let hotkeyErr = $state<Record<number, string | null>>({});

  const list = $derived(draft.triggers.map((t, i) => ({ t, i })).filter((x) => x.t.kind !== "voice"));

  function blank(kind: K): Trigger {
    switch (kind) {
      case "cron":
        return { kind, expr: "0 9 * * 1-5", prompt: "", catchUp: true };
      case "folder":
        return { kind, path: "", events: ["created"], glob: "", recursive: false, prompt: "" };
      case "mail":
        return { kind, accountId: "", from: "", subjectContains: "", prompt: "" };
      case "notification":
        return { kind, app: "", contains: "", prompt: "" };
      case "hotkey":
        return { kind, accelerator: "", input: "clipboard", prompt: "" };
    }
  }

  function add(kind: K) {
    draft.triggers = [...draft.triggers, blank(kind)];
    adding = false;
    if (kind === "cron") preview(draft.triggers.length - 1, "0 9 * * 1-5");
  }
  function remove(i: number) {
    draft.triggers = draft.triggers.filter((_, j) => j !== i);
  }
  function patch(i: number, p: Record<string, unknown>) {
    draft.triggers = draft.triggers.map((t, j) => (j === i ? ({ ...t, ...p } as Trigger) : t));
  }

  async function preview(i: number, expr: string) {
    try {
      const next = await api.cronPreview(expr);
      previews[i] = "Prossime: " + next.map((n) => listDate(n) + " " + new Date(n).toLocaleTimeString("it-IT", { hour: "2-digit", minute: "2-digit" })).join(" · ");
    } catch (e) {
      previews[i] = typeof e === "string" ? e : "Espressione non valida";
    }
  }

  async function pickFolder(i: number) {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const r = await open({ directory: true, title: "Cartella da osservare" });
      if (typeof r === "string") patch(i, { path: r });
    } catch {}
  }

  async function setHotkey(i: number, v: string) {
    hotkeyErr[i] = v ? await api.hotkeyCheck(v, draft.id).catch(() => null) : null;
    patch(i, { accelerator: hotkeyErr[i] ? "" : v });
  }

  $effect(() => {
    for (const { t, i } of list) if (t.kind === "cron" && previews[i] === undefined) preview(i, t.expr);
  });
</script>

<div class="list">
  {#each list as { t, i } (i)}
    {@const meta = KINDS.find((k) => k.kind === t.kind)}
    <div class="trig">
      <div class="th">
        <Icon name={meta?.icon ?? "bolt"} size={14} />
        <strong>{meta?.label}</strong>
        <button class="icon-btn" onclick={() => remove(i)} aria-label="Togli"><Icon name="x" size={13} /></button>
      </div>
      {#if t.kind === "cron"}
        <div class="two">
          <select class="input" value={CRON_PRESETS.some(([e]) => e === t.expr) ? t.expr : ""} onchange={(e) => { const v = (e.target as HTMLSelectElement).value; if (v) { patch(i, { expr: v }); preview(i, v); } }}>
            {#each CRON_PRESETS as [e, l] (e)}<option value={e}>{l}</option>{/each}
            <option value="">Personalizzato…</option>
          </select>
          <input class="input mono" value={t.expr} spellcheck="false" aria-label="Espressione cron" onchange={(e) => { const v = (e.target as HTMLInputElement).value.trim(); patch(i, { expr: v }); preview(i, v); }} />
        </div>
        {#if previews[i]}<small class="help">{previews[i]}</small>{/if}
        <label class="check"><input type="checkbox" checked={t.catchUp} onchange={(e) => patch(i, { catchUp: (e.target as HTMLInputElement).checked })} /><span>Se il computer era spento, recupera appena si accende (entro 12 ore)</span></label>
      {:else if t.kind === "folder"}
        <div class="pathrow">
          <input class="input" value={t.path} placeholder="~/Downloads" spellcheck="false" onchange={(e) => patch(i, { path: (e.target as HTMLInputElement).value })} />
          <button class="btn btn-line" onclick={() => pickFolder(i)}>Scegli</button>
        </div>
        <div class="two">
          <input class="input" value={t.glob} placeholder="Tutti i file (o es. *.pdf)" spellcheck="false" onchange={(e) => patch(i, { glob: (e.target as HTMLInputElement).value })} />
          <select class="input" value={t.events.join(",")} onchange={(e) => patch(i, { events: (e.target as HTMLSelectElement).value.split(",") })}>
            <option value="created">File nuovi</option>
            <option value="created,modified">Nuovi o modificati</option>
            <option value="deleted">File eliminati</option>
          </select>
        </div>
        <label class="check"><input type="checkbox" checked={t.recursive} onchange={(e) => patch(i, { recursive: (e.target as HTMLInputElement).checked })} /><span>Anche nelle sottocartelle</span></label>
        <small class="help">I file che arrivano insieme si raggruppano: l'agente parte una volta sola, con l'elenco.</small>
      {:else if t.kind === "mail"}
        <div class="two">
          <select class="input" value={t.accountId} onchange={(e) => patch(i, { accountId: (e.target as HTMLSelectElement).value })}>
            <option value="">Qualsiasi account</option>
            {#each accounts.list as a (a.id)}<option value={a.id}>{a.email}</option>{/each}
          </select>
          <input class="input" value={t.from} placeholder="Da chiunque (o nome, indirizzo)" onchange={(e) => patch(i, { from: (e.target as HTMLInputElement).value })} />
        </div>
        <input class="input" value={t.subjectContains} placeholder="Con qualsiasi oggetto (o testo che deve contenere)" onchange={(e) => patch(i, { subjectContains: (e.target as HTMLInputElement).value })} />
        <small class="help">Usa le email nuove raccolte per le notifiche (Impostazioni → Notifiche → Nuove email).</small>
      {:else if t.kind === "notification"}
        <div class="two">
          <input class="input" value={t.app} placeholder="Da qualsiasi app" onchange={(e) => patch(i, { app: (e.target as HTMLInputElement).value })} />
          <input class="input" value={t.contains} placeholder="Con qualsiasi testo" onchange={(e) => patch(i, { contains: (e.target as HTMLInputElement).value })} />
        </div>
      {:else if t.kind === "hotkey"}
        <div class="two">
          <HotkeyField value={t.accelerator} onchange={(v) => setHotkey(i, v)} />
          <select class="input" value={t.input} onchange={(e) => patch(i, { input: (e.target as HTMLSelectElement).value })}>
            <option value="clipboard">Con il testo copiato</option>
            <option value="palette">Apri la palette su questo agente</option>
            <option value="none">Parti subito</option>
          </select>
        </div>
        {#if hotkeyErr[i]}<small class="err">{hotkeyErr[i]}</small>{/if}
      {/if}
      {#if !(t.kind === "hotkey" && t.input === "palette")}
        <textarea class="input" rows="2" value={(t as { prompt?: string }).prompt ?? ""} placeholder="Cosa deve fare quando parte (es. «Rinomina i file per data e spostali in Archivio»)" onchange={(e) => patch(i, { prompt: (e.target as HTMLTextAreaElement).value })}></textarea>
      {/if}
    </div>
  {/each}

  {#if adding}
    <div class="kinds">
      {#each KINDS as k (k.kind)}
        <button class="btn btn-line" onclick={() => add(k.kind)}><Icon name={k.icon} size={14} /> {k.label}</button>
      {/each}
      <button class="btn btn-quiet" onclick={() => (adding = false)}>Annulla</button>
    </div>
  {:else}
    <button class="btn btn-line add" onclick={() => (adding = true)}><Icon name="plus" size={14} /> Aggiungi un avvio automatico</button>
  {/if}
  {#if list.some((x) => x.t.kind === "mail" || x.t.kind === "notification")}
    <p class="warn"><Icon name="shield" size={14} /> Email e notifiche arrivano da altri: l'agente le tratta come dati, e le azioni rischiose restano da approvare.</p>
  {/if}
</div>

<style>
  .list {
    display: grid;
    gap: 10px;
  }
  .trig {
    display: grid;
    gap: 8px;
    padding: 10px 12px 12px;
    border-radius: var(--r-md);
    box-shadow: inset 0 0 0 1px var(--rule);
  }
  .th {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--accent);
  }
  .th strong {
    flex: 1;
    font-size: var(--t-sm);
    font-weight: 650;
    color: var(--ink);
  }
  .two {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }
  .pathrow {
    display: flex;
    gap: 8px;
  }
  .mono {
    font-family: var(--font-mono, ui-monospace, monospace);
  }
  .check {
    display: flex;
    gap: 8px;
    align-items: center;
    font-size: var(--t-sm);
  }
  .check input {
    accent-color: var(--accent);
  }
  .kinds {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .add {
    justify-self: start;
  }
  .warn {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    margin: 0;
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
</style>
