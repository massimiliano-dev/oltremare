<script lang="ts">
  import { onMount } from "svelte";
  import Icon from "../Icon.svelte";
  import { accounts } from "../state/accounts.svelte";
  import { settings } from "../state/settings.svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";

  const s = $derived(settings.s!);
  let showIds = $state(false);
  onMount(() => accounts.refresh());

  async function saveIds(patch: Partial<typeof s>) {
    await settings.save(patch);
    await accounts.refresh();
  }
</script>

<section>
  {#each accounts.list as a (a.id)}
    <div class="acc">
      <span class="logo {a.provider}">{a.provider === "microsoft" ? "M" : "G"}</span>
      <span class="who"><strong>{a.email}</strong><small>{a.provider === "microsoft" ? "Microsoft 365 / Outlook" : "Google"} · posta, calendario, attività, contatti{a.teams ? " · Teams" : ""}</small></span>
      {#if a.provider === "microsoft" && !a.teams}
        <button class="btn btn-quiet" onclick={() => accounts.connect("microsoft", a.id)} disabled={!!accounts.connecting} title="Per le notifiche delle chat (account di lavoro o scuola)">Collega Teams</button>
      {/if}
      <button class="icon-btn" onclick={() => accounts.remove(a)} title="Scollega" aria-label="Scollega {a.email}"><Icon name="x" size={15} /></button>
    </div>
  {:else}
    <p class="empty">Collega un account per far leggere e scrivere posta, gestire il calendario e le attività agli agenti, e per il briefing del mattino.</p>
  {/each}

  {#if accounts.connecting}
    <div class="waiting">
      <span class="spin"></span>
      <span>Completa l'accesso nel browser…{#if accounts.url}<button class="link" onclick={() => openUrl(accounts.url!)}>Riapri la pagina</button>{/if}</span>
      <button class="btn btn-quiet" onclick={() => accounts.cancel()}>Annulla</button>
    </div>
  {:else}
    <div class="add">
      <button class="btn btn-line" disabled={!accounts.available.includes("microsoft")} onclick={() => accounts.connect("microsoft")}><span class="logo microsoft sm">M</span> Collega Microsoft</button>
      <button class="btn btn-line" disabled={!accounts.available.includes("google")} onclick={() => accounts.connect("google")}><span class="logo google sm">G</span> Collega Google</button>
    </div>
  {/if}
  {#if accounts.available.length < 2}
    <small class="help">Per collegare un account serve l'ID di un'applicazione registrata da te (gratuita): <button class="link" onclick={() => (showIds = true)}>inseriscilo qui</button>. Le istruzioni sono nel README.</small>
  {/if}
</section>

<section>
  <button class="toggle" onclick={() => (showIds = !showIds)}><Icon name={showIds ? "down" : "chevron"} size={13} /> Applicazioni OAuth</button>
  {#if showIds}
    <label class="field">
      <span>Microsoft · ID applicazione (client)</span>
      <input class="input" value={s.msClientId} placeholder="00000000-0000-0000-0000-000000000000" spellcheck="false" onchange={(e) => saveIds({ msClientId: (e.target as HTMLInputElement).value.trim() })} />
      <small class="help">Entra ID → Registrazioni app → client pubblico, redirect http://localhost.</small>
    </label>
    <label class="field">
      <span>Google · ID client</span>
      <input class="input" value={s.googleClientId} placeholder="….apps.googleusercontent.com" spellcheck="false" onchange={(e) => saveIds({ googleClientId: (e.target as HTMLInputElement).value.trim() })} />
    </label>
    <label class="field">
      <span>Google · secret client</span>
      <input class="input" value={s.googleClientSecret} spellcheck="false" onchange={(e) => saveIds({ googleClientSecret: (e.target as HTMLInputElement).value.trim() })} />
      <small class="help">Per le app desktop Google non considera segreto questo valore. Tipo di client: App desktop.</small>
    </label>
  {/if}
</section>

<style>
  section {
    display: grid;
    gap: 10px;
    padding: 20px 0 22px;
    border-bottom: 1px solid var(--rule-soft);
  }
  section:last-child {
    border-bottom: 0;
  }
  .acc {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 10px 10px 12px;
    border-radius: var(--r-md);
    box-shadow: inset 0 0 0 1px var(--rule);
  }
  .logo {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: 8px;
    font-weight: 800;
    color: white;
    flex: none;
  }
  .logo.sm {
    width: 18px;
    height: 18px;
    border-radius: 5px;
    font-size: 0.65rem;
  }
  .logo.microsoft {
    background: oklch(0.55 0.15 245);
  }
  .logo.google {
    background: oklch(0.6 0.17 145);
  }
  .who {
    flex: 1;
    min-width: 0;
    display: grid;
  }
  .who strong {
    font-size: var(--t-md);
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .who small {
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
  .empty {
    margin: 0;
    font-size: var(--t-sm);
    line-height: 1.5;
    color: var(--ink-2);
  }
  .add {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .waiting {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    border-radius: var(--r-md);
    background: var(--accent-mist);
    font-size: var(--t-sm);
  }
  .waiting span:nth-child(2) {
    flex: 1;
    display: flex;
    gap: 8px;
  }
  .spin {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 2px solid var(--accent);
    border-right-color: transparent;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .toggle {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--t-xs);
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--accent);
    justify-self: start;
  }
</style>
