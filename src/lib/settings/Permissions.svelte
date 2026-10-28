<script lang="ts">
  // Permessi di macOS che servono agli strumenti: si controllano e si aprono le Impostazioni giuste.
  import { onMount } from "svelte";
  import Icon from "../Icon.svelte";
  import { api, type Permissions } from "../api";

  let { only }: { only?: ("accessibility" | "screen" | "fullDisk")[] } = $props();
  let p = $state<Permissions | null>(null);
  const ROWS = [
    { id: "screen" as const, label: "Registrazione schermo", why: "per vedere lo schermo" },
    { id: "accessibility" as const, label: "Accessibilità", why: "per usare mouse e tastiera" },
    { id: "fullDisk" as const, label: "Accesso completo al disco", why: "per leggere le notifiche di sistema" },
  ];
  const rows = $derived(ROWS.filter((r) => !only || only.includes(r.id)));

  async function refresh() {
    p = await api.permissions().catch(() => null);
  }
  onMount(() => {
    refresh();
    // Quando torni da Impostazioni di Sistema si ricontrolla
    const f = () => refresh();
    window.addEventListener("focus", f);
    return () => window.removeEventListener("focus", f);
  });
</script>

{#if p?.needed}
  <div class="perms">
    {#each rows as r (r.id)}
      {@const ok = p[r.id]}
      <div class="row" class:ok>
        <Icon name={ok ? "check" : "shield"} size={15} />
        <span><strong>{r.label}</strong><small>{r.why}</small></span>
        {#if ok}<em>Concesso</em>{:else}<button class="btn btn-line" onclick={() => api.requestPermission(r.id)}>Concedi</button>{/if}
      </div>
    {/each}
    <small class="help">
      Si concedono in Impostazioni di Sistema → Privacy e sicurezza; alcuni richiedono di riaprire Oltremare.
      Se l'interruttore di Oltremare è già acceso ma qui risulta da concedere, il permesso appartiene a una versione precedente:
      selezionalo, toglilo con «–» e premi di nuovo «Concedi».
    </small>
  </div>
{/if}

<style>
  .perms {
    display: grid;
    gap: 6px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    border-radius: var(--r-md);
    background: var(--paper-2);
    color: var(--warn-ink);
  }
  .row.ok {
    color: var(--ok);
  }
  .row span {
    flex: 1;
    display: grid;
  }
  .row strong {
    font-size: var(--t-sm);
    font-weight: 600;
    color: var(--ink);
  }
  .row small {
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
  em {
    font-style: normal;
    font-size: var(--t-xs);
    font-weight: 650;
  }
</style>
