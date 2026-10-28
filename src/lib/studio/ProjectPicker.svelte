<script lang="ts">
  // La schermata d'ingresso dello Studio: i progetti di Sviluppo e Personale, da scegliere a colpo d'occhio.
  import { onMount } from "svelte";
  import Icon from "../Icon.svelte";
  import { studio } from "../state/studio.svelte";
  import type { StudioProject } from "../api";

  let query = $state("");
  let filter = $state<"tutti" | "Sviluppo" | "Personale" | "modifiche">("tutti");
  let search: HTMLInputElement | undefined = $state();

  onMount(() => {
    if (!studio.projects.length) studio.loadProjects();
    search?.focus();
  });

  function quando(ms: number) {
    const d = Date.now() - ms;
    if (d < 3600_000) return `${Math.max(1, Math.round(d / 60_000))} min fa`;
    if (d < 86400_000) return `${Math.round(d / 3600_000)} h fa`;
    if (d < 30 * 86400_000) return `${Math.round(d / 86400_000)} g fa`;
    return new Date(ms).toLocaleDateString("it-IT", { month: "short", year: "numeric" });
  }

  const iniziali = (n: string) =>
    n
      .replace(/[_-]+/g, " ")
      .split(/\s+/)
      .filter(Boolean)
      .slice(0, 2)
      .map((w) => w[0]?.toUpperCase())
      .join("");

  const filtrati = $derived(
    studio.projects
      .filter((p) => (filter === "tutti" ? true : filter === "modifiche" ? p.changes > 0 : p.root === filter))
      .filter((p) => !query.trim() || `${p.name} ${p.group} ${p.stack.join(" ")} ${p.readme ?? ""}`.toLowerCase().includes(query.trim().toLowerCase()))
      .sort((a, b) => Number(studio.pinned.includes(b.path)) - Number(studio.pinned.includes(a.path)) || b.updatedAt - a.updatedAt),
  );
  const recenti = $derived(
    studio.recent
      .map((p) => studio.projects.find((x) => x.path === p))
      .filter((x): x is StudioProject => !!x)
      .slice(0, 4),
  );
  const conteggi = $derived({
    tutti: studio.projects.length,
    Sviluppo: studio.projects.filter((p) => p.root === "Sviluppo").length,
    Personale: studio.projects.filter((p) => p.root === "Personale").length,
    modifiche: studio.projects.filter((p) => p.changes > 0).length,
  });

  function key(e: KeyboardEvent) {
    if (e.key === "Enter" && filtrati[0]) studio.open(filtrati[0]);
  }
</script>

<div class="picker">
  <header class="hero">
    <p class="eyebrow">Oltremare Studio</p>
    <h1>Su cosa lavoriamo?</h1>
    <div class="searchbar">
      <Icon name="search" size={17} />
      <input bind:this={search} bind:value={query} onkeydown={key} placeholder="Cerca un progetto, uno stack, una descrizione…" spellcheck="false" />
      <kbd>↵</kbd>
    </div>
    <div class="filters" role="radiogroup" aria-label="Filtra i progetti">
      {#each [["tutti", "Tutti"], ["Sviluppo", "Sviluppo"], ["Personale", "Personale"], ["modifiche", "Con modifiche"]] as [id, label] (id)}
        <button role="radio" aria-checked={filter === id} class:on={filter === id} onclick={() => (filter = id as typeof filter)}>
          {label}<span>{conteggi[id as keyof typeof conteggi]}</span>
        </button>
      {/each}
      <button class="refresh" onclick={() => studio.loadProjects()} disabled={studio.loading} title="Aggiorna"><Icon name="refresh" size={14} /></button>
    </div>
  </header>

  {#if studio.loading && !studio.projects.length}
    <div class="grid">
      {#each Array(8) as _, i (i)}<div class="card skeleton" style="height:190px"></div>{/each}
    </div>
  {:else}
    {#if recenti.length && !query && filter === "tutti"}
      <section>
        <h2>Ripresi di recente</h2>
        <div class="recent">
          {#each recenti as p (p.path)}
            <button class="rcard" style="--h:{p.hue}" onclick={() => studio.open(p)}>
              <span class="ricon">{#if p.icon}<img src={p.icon} alt="" />{:else}{iniziali(p.name)}{/if}</span>
              <span class="rtxt"><strong>{p.name}</strong><small>{p.stack.slice(0, 3).join(" · ") || p.root}</small></span>
              <Icon name="chevron" size={14} />
            </button>
          {/each}
        </div>
      </section>
    {/if}

    <section>
      <h2>{filter === "tutti" ? "Tutti i progetti" : filter === "modifiche" ? "Con modifiche non salvate in git" : filter} <span class="n">{filtrati.length}</span></h2>
      <div class="grid">
        {#each filtrati as p (p.path)}
          <div class="card" style="--h:{p.hue}">
            <button class="open" onclick={() => studio.open(p)} aria-label="Apri {p.name}"></button>
            <div class="top">
              <span class="icon">{#if p.icon}<img src={p.icon} alt="" />{:else}{iniziali(p.name)}{/if}</span>
              <span class="title">
                <strong>{p.name}</strong>
                <small>{p.group ? `${p.root} / ${p.group}` : p.root}</small>
              </span>
              <button class="pin" class:on={studio.pinned.includes(p.path)} onclick={() => studio.togglePin(p.path)} aria-label="Fissa in alto" title="Fissa in alto"><Icon name="star" size={15} /></button>
            </div>
            {#if p.readme}<p class="desc">{p.readme}</p>{/if}
            {#if p.stack.length}
              <div class="chips">{#each p.stack as s (s)}<span>{s}</span>{/each}</div>
            {/if}
            <div class="foot">
              {#if p.branch}<span class="br"><Icon name="layers" size={12} /> {p.branch}</span>{/if}
              {#if p.changes > 0}<span class="ch"><span class="dot"></span>{p.changes} modifiche</span>{/if}
              <span class="when">{quando(p.updatedAt)}</span>
            </div>
          </div>
        {:else}
          <p class="empty">Nessun progetto con «{query}».</p>
        {/each}
      </div>
    </section>
  {/if}
</div>

<style>
  .picker {
    flex: 1;
    overflow-y: auto;
    padding: 0 40px 48px;
    background:
      radial-gradient(1200px 380px at 10% -10%, oklch(0.9 0.06 var(--accent-h) / 0.55), transparent 60%),
      radial-gradient(900px 300px at 95% 0%, oklch(0.9 0.05 320 / 0.35), transparent 60%),
      var(--paper);
  }
  :global([data-theme="dark"]) .picker {
    background:
      radial-gradient(1200px 380px at 10% -10%, oklch(0.32 0.1 var(--accent-h) / 0.6), transparent 60%),
      radial-gradient(900px 300px at 95% 0%, oklch(0.3 0.08 320 / 0.35), transparent 60%),
      var(--paper);
  }
  .hero {
    max-width: 880px;
    margin: 0 auto;
    padding: 56px 0 28px;
    text-align: center;
  }
  .eyebrow {
    margin: 0 0 6px;
    font-size: var(--t-xs);
    font-weight: 700;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--accent);
  }
  h1 {
    margin: 0 0 22px;
    font-family: var(--font-serif);
    font-weight: 400;
    font-size: clamp(2.4rem, 1.6rem + 2vw, 3.4rem);
    line-height: 1;
    letter-spacing: -0.01em;
  }
  .searchbar {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 52px;
    padding: 0 16px;
    border-radius: 16px;
    background: var(--paper);
    box-shadow: 0 0 0 1px var(--rule), 0 14px 36px -18px oklch(0.3 0.12 var(--accent-h) / 0.35);
    color: var(--ink-3);
  }
  .searchbar input {
    flex: 1;
    border: 0;
    outline: none;
    background: transparent;
    font-size: 1.05rem;
    color: var(--ink);
  }
  .searchbar kbd {
    font-family: inherit;
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
  .filters {
    display: flex;
    justify-content: center;
    gap: 6px;
    margin-top: 14px;
    flex-wrap: wrap;
  }
  .filters button {
    display: flex;
    align-items: center;
    gap: 7px;
    height: 32px;
    padding: 0 13px;
    border-radius: 999px;
    font-size: var(--t-sm);
    color: var(--ink-2);
    background: color-mix(in oklch, var(--paper) 70%, transparent);
    box-shadow: inset 0 0 0 1px var(--rule);
  }
  .filters button span {
    font-size: 0.7rem;
    color: var(--ink-3);
    font-variant-numeric: tabular-nums;
  }
  .filters button.on {
    color: var(--on-accent);
    background: var(--accent);
    box-shadow: none;
  }
  .filters button.on span {
    color: color-mix(in oklch, var(--on-accent) 75%, transparent);
  }
  .filters .refresh {
    width: 32px;
    padding: 0;
    justify-content: center;
  }
  section {
    max-width: 1320px;
    margin: 0 auto 28px;
  }
  h2 {
    display: flex;
    align-items: baseline;
    gap: 8px;
    margin: 0 0 12px;
    font-size: var(--t-xs);
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--ink-3);
  }
  h2 .n {
    font-weight: 600;
    letter-spacing: 0;
  }
  .recent {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(250px, 1fr));
    gap: 10px;
  }
  .rcard {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 14px;
    border-radius: 14px;
    text-align: left;
    color: var(--ink-3);
    background: linear-gradient(135deg, oklch(0.62 0.15 var(--h) / 0.16), transparent 70%), var(--paper);
    box-shadow: inset 0 0 0 1px var(--rule);
    transition: transform 0.15s, box-shadow 0.15s;
  }
  .rcard:hover {
    transform: translateY(-2px);
    box-shadow: inset 0 0 0 1px oklch(0.6 0.15 var(--h) / 0.5), 0 10px 24px -14px oklch(0.4 0.15 var(--h) / 0.6);
  }
  .rtxt {
    flex: 1;
    min-width: 0;
    display: grid;
  }
  .rtxt strong {
    color: var(--ink);
    font-weight: 650;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .rtxt small {
    font-size: var(--t-xs);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 14px;
  }
  .card {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 16px;
    border-radius: 16px;
    background: var(--paper);
    box-shadow: inset 0 0 0 1px var(--rule);
    transition: transform 0.18s cubic-bezier(0.2, 0.8, 0.2, 1), box-shadow 0.18s;
    overflow: hidden;
  }
  .card::before {
    content: "";
    position: absolute;
    inset: 0 0 auto 0;
    height: 3px;
    background: linear-gradient(90deg, oklch(0.62 0.17 var(--h)), oklch(0.7 0.14 calc(var(--h) + 40)));
    opacity: 0.85;
  }
  .card:hover {
    transform: translateY(-3px);
    box-shadow: inset 0 0 0 1px oklch(0.6 0.15 var(--h) / 0.45), 0 18px 34px -20px oklch(0.4 0.15 var(--h) / 0.65);
  }
  .open {
    position: absolute;
    inset: 0;
    z-index: 1;
    border-radius: inherit;
  }
  .top {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .icon,
  .ricon {
    flex: none;
    display: grid;
    place-items: center;
    width: 42px;
    height: 42px;
    border-radius: 12px;
    font-weight: 750;
    font-size: 0.95rem;
    color: white;
    background: linear-gradient(140deg, oklch(0.62 0.17 var(--h)), oklch(0.45 0.16 calc(var(--h) + 30)));
    overflow: hidden;
  }
  .ricon {
    width: 34px;
    height: 34px;
    border-radius: 10px;
    font-size: 0.8rem;
  }
  .icon img,
  .ricon img {
    width: 100%;
    height: 100%;
    object-fit: contain;
    background: var(--paper);
  }
  .title {
    flex: 1;
    min-width: 0;
    display: grid;
  }
  .title strong {
    font-size: 1.02rem;
    font-weight: 680;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .title small {
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
  .pin {
    position: relative;
    z-index: 2;
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: 8px;
    color: var(--ink-3);
    opacity: 0;
    transition: opacity 0.15s;
  }
  .card:hover .pin,
  .pin.on {
    opacity: 1;
  }
  .pin.on {
    color: oklch(0.75 0.15 80);
  }
  .desc {
    margin: 0;
    font-size: var(--t-sm);
    color: var(--ink-2);
    line-height: 1.45;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }
  .chips span {
    padding: 2px 8px;
    border-radius: 999px;
    font-size: 0.7rem;
    font-weight: 600;
    color: oklch(0.45 0.13 var(--h));
    background: oklch(0.62 0.15 var(--h) / 0.12);
  }
  :global([data-theme="dark"]) .chips span {
    color: oklch(0.82 0.1 var(--h));
  }
  .foot {
    margin-top: auto;
    display: flex;
    align-items: center;
    gap: 12px;
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
  .br {
    display: flex;
    align-items: center;
    gap: 4px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 45%;
  }
  .ch {
    display: flex;
    align-items: center;
    gap: 5px;
    color: oklch(0.6 0.13 70);
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: currentColor;
  }
  .when {
    margin-left: auto;
  }
  .empty {
    color: var(--ink-3);
  }
</style>
