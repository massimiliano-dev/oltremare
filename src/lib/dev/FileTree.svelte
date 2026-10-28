<script lang="ts">
  // Albero dei file: icone colorate per tipo e ruolo, guide di rientro, cartelle che si aprono scorrendo.
  import { slide } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import FileIcon from "./FileIcon.svelte";
  import FileTree from "./FileTree.svelte";
  import { dev } from "../state/dev.svelte";

  let { path, depth = 0, onopen, active = null }: { path: string; depth?: number; onopen?: (p: string) => void; active?: string | null } = $props();
  const entries = $derived(dev.tree[path] ?? []);
  const apri = (p: string) => (onopen ? onopen(p) : dev.openFile(p));
  const corrente = $derived(active ?? dev.file?.path ?? null);
  const STATUS: Record<string, string> = { M: "modificato", A: "aggiunto", D: "eliminato", "??": "nuovo", R: "rinominato" };
  const ridotto = typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;
  // Una cartella è «cambiata» se contiene file cambiati
  const cambiate = (p: string) => [...dev.changed.keys()].filter((c) => c.startsWith(p + "/") || c.startsWith(p + "\\")).length;
</script>

<ul class="level" style="--d:{depth}" role={depth === 0 ? "tree" : "group"}>
  {#each entries as e (e.path)}
    {@const st = dev.changed.get(e.path)}
    <li role="treeitem" aria-expanded={e.dir ? !!dev.expanded[e.path] : undefined} aria-selected={!e.dir && corrente === e.path}>
      {#if e.dir}
        {@const n = cambiate(e.path)}
        <button class="row dir" class:open={dev.expanded[e.path]} onclick={() => dev.toggleDir(e.path)}>
          <svg class="chev" width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M3.5 2 6.5 5l-3 3" /></svg>
          <FileIcon name={e.name} folder open={!!dev.expanded[e.path]} />
          <span class="name">{e.name}</span>
          {#if n}<span class="cnt" title="{n} file cambiati">{n}</span>{/if}
        </button>
        {#if dev.expanded[e.path]}
          <div transition:slide={{ duration: ridotto ? 0 : 160, easing: cubicOut }}>
            {#if dev.tree[e.path] && !dev.tree[e.path].length}
              <p class="vuota" style="--d:{depth + 1}">cartella vuota</p>
            {:else}
              <FileTree path={e.path} depth={depth + 1} {onopen} {active} />
            {/if}
          </div>
        {/if}
      {:else}
        <button class="row file" class:on={corrente === e.path} class:ch={!!st} onclick={() => apri(e.path)} title={st ? `${e.name} · ${STATUS[st] ?? st}` : e.name}>
          <span class="chev-sp"></span>
          <FileIcon name={e.name} />
          <span class="name">{e.name}</span>
          {#if st}<span class="st st-{st === '??' ? 'n' : st[0]}">{st === "??" ? "N" : st[0]}</span>{/if}
        </button>
      {/if}
    </li>
  {/each}
</ul>

<style>
  .level {
    position: relative;
    list-style: none;
    margin: 0;
    padding: 0;
  }
  /* Guida di rientro: una linea sottile sotto la cartella aperta */
  .level:not([style*="--d:0"])::before {
    content: "";
    position: absolute;
    top: 2px;
    bottom: 6px;
    left: calc(var(--d) * 14px - 1px);
    width: 1px;
    background: var(--rule-soft);
    border-radius: 1px;
  }
  .row {
    position: relative;
    width: 100%;
    display: flex;
    align-items: center;
    gap: 6px;
    height: 26px;
    padding: 0 8px 0 calc(6px + var(--d) * 14px);
    border-radius: 8px;
    font-size: var(--t-sm);
    text-align: left;
    color: var(--ink-2);
    white-space: nowrap;
    transition: background-color 0.12s, color 0.12s;
  }
  .row:hover {
    background: color-mix(in oklch, var(--accent) 7%, transparent);
    color: var(--ink);
  }
  .row:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  .row.on {
    background: color-mix(in oklch, var(--accent) 15%, transparent);
    color: var(--ink);
    font-weight: 600;
  }
  .dir {
    color: var(--ink);
    font-weight: 550;
  }
  .chev {
    flex: none;
    margin-right: -1px;
    color: var(--ink-3);
    transition: transform 0.16s cubic-bezier(0.2, 0.8, 0.2, 1);
  }
  .chev path {
    fill: none;
    stroke: currentColor;
    stroke-width: 1.4;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .dir.open .chev {
    transform: rotate(90deg);
    color: var(--accent);
  }
  .chev-sp {
    width: 9px;
    flex: none;
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .ch .name {
    color: var(--ink);
  }
  .cnt {
    flex: none;
    min-width: 16px;
    height: 16px;
    padding: 0 4px;
    border-radius: 999px;
    display: grid;
    place-items: center;
    font-size: 0.62rem;
    font-weight: 700;
    color: oklch(0.5 0.13 70);
    background: oklch(0.8 0.12 80 / 0.28);
  }
  .st {
    flex: none;
    width: 16px;
    height: 16px;
    border-radius: 5px;
    display: grid;
    place-items: center;
    font-size: 0.6rem;
    font-weight: 800;
  }
  .st-M,
  .st-R {
    color: oklch(0.5 0.13 70);
    background: oklch(0.8 0.12 80 / 0.3);
  }
  .st-A,
  .st-n {
    color: oklch(0.45 0.13 150);
    background: oklch(0.78 0.13 150 / 0.28);
  }
  .st-D {
    color: oklch(0.5 0.17 25);
    background: oklch(0.75 0.14 25 / 0.25);
  }
  :global([data-theme="dark"]) .st-M,
  :global([data-theme="dark"]) .st-R,
  :global([data-theme="dark"]) .cnt {
    color: oklch(0.85 0.12 80);
  }
  :global([data-theme="dark"]) .st-A,
  :global([data-theme="dark"]) .st-n {
    color: oklch(0.85 0.13 150);
  }
  :global([data-theme="dark"]) .st-D {
    color: oklch(0.82 0.13 25);
  }
  .vuota {
    margin: 0;
    padding: 3px 0 5px calc(28px + var(--d) * 14px);
    font-size: var(--t-xs);
    font-style: italic;
    color: var(--ink-3);
  }
</style>
