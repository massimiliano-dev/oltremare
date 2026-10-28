<script lang="ts">
  import { onMount } from "svelte";
  import Icon from "../Icon.svelte";
  import Permissions from "./Permissions.svelte";
  import { api, errorText, type NotificationRule } from "../api";
  import { settings } from "../state/settings.svelte";
  import { notifications } from "../state/notifications.svelte";
  import { agents } from "../state/agents.svelte";
  import { runs } from "../state/runs.svelte";
  import { ui } from "../state/ui.svelte";

  const s = $derived(settings.s!);
  const src = $derived(notifications.sources);
  let slackToken = $state("");
  let slackBusy = $state(false);
  const DAYS = ["L", "M", "M", "G", "V", "S", "D"];
  const DAY_NAMES = ["lunedì", "martedì", "mercoledì", "giovedì", "venerdì", "sabato", "domenica"];

  onMount(() => notifications.loadSources());

  function toggleDay(d: number) {
    const set = new Set(s.briefingDays);
    if (set.has(d)) set.delete(d);
    else set.add(d);
    settings.save({ briefingDays: [...set].sort() });
  }

  async function saveSlack(token: string) {
    slackBusy = true;
    try {
      const who = await api.slackTokenSet(token);
      slackToken = "";
      if (who) {
        ui.toast(`Slack collegato: ${who}`);
        if (!s.notificationsSlack) settings.save({ notificationsSlack: true });
      }
      await notifications.loadSources();
    } catch (e) {
      ui.toast(errorText(e), "error");
    } finally {
      slackBusy = false;
    }
  }

  function setRules(rules: NotificationRule[]) {
    settings.save({ notificationRules: rules });
  }
  function updateRule(i: number, patch: Partial<NotificationRule>) {
    setRules(s.notificationRules.map((r, j) => (j === i ? { ...r, ...patch } : r)));
  }
  function addRule() {
    setRules([...s.notificationRules, { id: crypto.randomUUID(), app: "", contains: "", action: "alert", agentId: "", enabled: true }]);
  }

  async function tryBriefing() {
    const run = await notifications.briefingNow();
    if (run) {
      ui.showSettings = false;
      runs.openRun(run.agentId, run.conversationId);
    }
  }
</script>

<section>
  <h3>Briefing del mattino</h3>
  <label class="check">
    <input type="checkbox" checked={s.briefingEnabled} onchange={(e) => settings.save({ briefingEnabled: (e.target as HTMLInputElement).checked })} />
    <span><strong>Leggimi il briefing ogni mattina</strong><small>Impegni, cose da fare, email da leggere e notifiche della sera prima, letti a voce. Se il computer era spento, appena lo accendi (entro le 13).</small></span>
  </label>
  {#if s.briefingEnabled}
    <div class="row">
      <label class="field time">
        <span>Alle</span>
        <input class="input" type="time" value={s.briefingTime} onchange={(e) => settings.save({ briefingTime: (e.target as HTMLInputElement).value || "08:00" })} />
      </label>
      <div class="field">
        <span>Nei giorni</span>
        <div class="days" role="group" aria-label="Giorni del briefing">
          {#each DAYS as d, i (i)}
            <button class:on={s.briefingDays.includes(i + 1)} onclick={() => toggleDay(i + 1)} aria-pressed={s.briefingDays.includes(i + 1)} title={DAY_NAMES[i]}>{d}</button>
          {/each}
        </div>
      </div>
    </div>
  {/if}
  <div class="row-btns">
    <button class="btn btn-line" onclick={tryBriefing} disabled={notifications.briefingBusy}><Icon name="play" size={13} /> {notifications.briefingBusy ? "Raccolgo i dati…" : "Fammi il briefing adesso"}</button>
    {#if agents.byId("briefing")}
      <button class="btn btn-quiet" onclick={() => { ui.showSettings = false; ui.editing = $state.snapshot(agents.byId("briefing")!); }}>Cambia modello e istruzioni</button>
    {/if}
  </div>
</section>

<section>
  <h3>Da dove leggere le notifiche</h3>
  <label class="check">
    <input type="checkbox" checked={s.notificationsMail} onchange={(e) => settings.save({ notificationsMail: (e.target as HTMLInputElement).checked })} />
    <span><strong>Nuove email</strong><small>Dagli account collegati, ogni due minuti.</small></span>
  </label>
  <label class="check">
    <input type="checkbox" checked={s.notificationsTeams} onchange={(e) => settings.save({ notificationsTeams: (e.target as HTMLInputElement).checked })} />
    <span>
      <strong>Microsoft Teams</strong>
      <small>
        {#if src?.teamsAccounts.length}Chat di {src.teamsAccounts.join(", ")}.{:else}Nessun account con Teams: in <button class="link" onclick={() => (ui.settingsTab = "accounts")}>Account</button> premi «Collega Teams» (account di lavoro o scuola).{/if}
      </small>
    </span>
  </label>
  {#if src?.systemSupported}
    <label class="check">
      <input type="checkbox" checked={s.notificationsSystem} onchange={(e) => settings.save({ notificationsSystem: (e.target as HTMLInputElement).checked })} />
      <span><strong>Centro Notifiche di macOS</strong><small>Tutte le app: WhatsApp, Messaggi, calendario… Serve l'accesso completo al disco, che Oltremare usa solo per leggere l'archivio delle notifiche.</small></span>
    </label>
    {#if s.notificationsSystem && !src.fullDisk}
      <Permissions only={["fullDisk"]} />
    {/if}
  {/if}
  <div class="check">
    <input type="checkbox" checked={s.notificationsSlack} disabled={!src?.slack} onchange={(e) => settings.save({ notificationsSlack: (e.target as HTMLInputElement).checked })} />
    <span>
      <strong>Slack</strong>
      {#if src?.slack}
        <small>Collegato come {src.slack}. <button class="link" onclick={() => saveSlack("")}>Scollega</button></small>
      {:else}
        <small>Crea un'app su api.slack.com/apps con gli ambiti utente <code>channels:history</code>, <code>groups:history</code>, <code>im:history</code>, <code>mpim:history</code>, <code>users:read</code>, installala e incolla qui lo «User OAuth Token».</small>
        <span class="key-row">
          <input class="input" type="password" bind:value={slackToken} placeholder="xoxp-…" spellcheck="false" autocomplete="off" />
          <button class="btn btn-primary" disabled={!slackToken.trim() || slackBusy} onclick={() => saveSlack(slackToken)}>{slackBusy ? "Verifico…" : "Collega"}</button>
        </span>
      {/if}
    </span>
  </div>
</section>

<section>
  <h3>Avvisi</h3>
  <p class="help">Le regole decidono cosa merita la tua attenzione. Senza regole le notifiche si raccolgono in silenzio per il briefing e per gli agenti.</p>
  {#each s.notificationRules as r, i (r.id)}
    <div class="rule" class:off={!r.enabled}>
      <input type="checkbox" checked={r.enabled} onchange={(e) => updateRule(i, { enabled: (e.target as HTMLInputElement).checked })} aria-label="Regola attiva" />
      <div class="rule-body">
        <div class="two">
          <input class="input" value={r.app} placeholder="Da qualsiasi app" onchange={(e) => updateRule(i, { app: (e.target as HTMLInputElement).value })} aria-label="App" />
          <input class="input" value={r.contains} placeholder="Con qualsiasi testo" onchange={(e) => updateRule(i, { contains: (e.target as HTMLInputElement).value })} aria-label="Contiene" />
        </div>
        <div class="two">
          <select class="input" value={r.action} onchange={(e) => updateRule(i, { action: (e.target as HTMLSelectElement).value as NotificationRule["action"] })} aria-label="Azione">
            <option value="alert">Avvisami</option>
            <option value="speak">Leggimela a voce</option>
            <option value="agent">Passala a un agente</option>
          </select>
          {#if r.action === "agent"}
            <select class="input" value={r.agentId} onchange={(e) => updateRule(i, { agentId: (e.target as HTMLSelectElement).value })} aria-label="Agente">
              <option value="" disabled>Scegli l'agente</option>
              {#each agents.list as a (a.id)}<option value={a.id}>{a.name}</option>{/each}
            </select>
          {/if}
        </div>
      </div>
      <button class="icon-btn" onclick={() => setRules(s.notificationRules.filter((_, j) => j !== i))} aria-label="Elimina regola"><Icon name="x" size={14} /></button>
    </div>
  {/each}
  <button class="btn btn-line add" onclick={addRule}><Icon name="plus" size={14} /> Aggiungi regola</button>
  <small class="help">Esempio: app «Teams», testo «urgente», «Leggimela a voce».</small>
  <div class="row">
    <label class="field time">
      <span>Silenzio dalle</span>
      <input class="input" type="time" value={s.quietFrom} onchange={(e) => settings.save({ quietFrom: (e.target as HTMLInputElement).value })} />
    </label>
    <label class="field time">
      <span>alle</span>
      <input class="input" type="time" value={s.quietTo} onchange={(e) => settings.save({ quietTo: (e.target as HTMLInputElement).value })} />
    </label>
  </div>
  <small class="help">In queste ore niente avvisi né voce. Le regole che avviano un agente funzionano comunque.</small>
</section>

<section>
  <h3>Privacy</h3>
  <label class="check">
    <input type="checkbox" checked={s.notificationsLocalOnly} onchange={(e) => settings.save({ notificationsLocalOnly: (e.target as HTMLInputElement).checked })} />
    <span><strong>Solo l'AI locale legge le notifiche</strong><small>Se lo attivi, Claude e GPT non vedono le notifiche: le leggono solo gli agenti con AI locale, e il briefing le include solo se usa l'AI locale. Spento, funzionano con qualsiasi modello. Le notifiche si conservano 30 giorni.</small></span>
  </label>
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
  .check {
    display: flex;
    align-items: flex-start;
    gap: 10px;
  }
  .check input[type="checkbox"] {
    margin-top: 3px;
    accent-color: var(--accent);
  }
  .check > span {
    flex: 1;
    display: grid;
    gap: 3px;
  }
  .check strong {
    font-weight: 600;
  }
  .check small {
    font-size: var(--t-xs);
    color: var(--ink-3);
    line-height: 1.45;
  }
  code {
    font-size: 0.92em;
  }
  .row {
    display: flex;
    gap: 14px;
    align-items: flex-end;
    flex-wrap: wrap;
  }
  .time .input {
    width: 120px;
  }
  .days {
    display: flex;
    gap: 4px;
  }
  .days button {
    width: 32px;
    height: 32px;
    border-radius: 50%;
    font-size: var(--t-xs);
    font-weight: 700;
    color: var(--ink-3);
    box-shadow: inset 0 0 0 1px var(--rule);
  }
  .days button.on {
    color: white;
    background: var(--accent);
    box-shadow: none;
  }
  .row-btns {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .key-row {
    display: flex;
    gap: 8px;
    margin-top: 4px;
  }
  .key-row .input {
    flex: 1;
  }
  .rule {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 10px;
    border-radius: var(--r-md);
    box-shadow: inset 0 0 0 1px var(--rule);
  }
  .rule.off {
    opacity: 0.6;
  }
  .rule > input {
    margin-top: 10px;
    accent-color: var(--accent);
  }
  .rule-body {
    flex: 1;
    display: grid;
    gap: 8px;
    min-width: 0;
  }
  .two {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }
  .add {
    justify-self: start;
  }
</style>
