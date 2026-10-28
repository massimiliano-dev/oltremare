<script lang="ts">
  import Icon from "../Icon.svelte";
  import FileTree from "./FileTree.svelte";
  import { dev } from "../state/dev.svelte";

  let { path, depth = 0 }: { path: string; depth?: number } = $props();
  const entries = $derived(dev.tree[path] ?? []);
  const STATUS: Record<string, string> = { M: "modificato", A: "aggiunto", D: "eliminato", "??": "nuovo", R: "rinominato" };
  // Una cartella è «cambiata» se contiene file cambiati
  const dirChanged = (p: string) => [...dev.changed.keys()].some((c) => c.startsWith(p + "/") || c.startsWith(p + "\\"));
</script>

{#each entries as e (e.path)}
  {@const st = dev.changed.get(e.path)}
  {#if e.dir}
    <button class="row dir" class:ch={dirChanged(e.path)} style="padding-left:{8 + depth * 14}px" onclick={() => dev.toggleDir(e.path)}>
      <Icon name={dev.expanded[e.path] ? "down" : "chevron"} size={11} />
      <Icon name="folder" size={13} />
      <span>{e.name}</span>
    </button>
    {#if dev.expanded[e.path]}
      <FileTree path={e.path} depth={depth + 1} />
    {/if}
  {:else}
    <button class="row" class:on={dev.file?.path === e.path} class:ch={!!st} style="padding-left:{25 + depth * 14}px" onclick={() => dev.openFile(e.path)} title={st ? STATUS[st] ?? st : e.name}>
      <span class="name">{e.name}</span>
      {#if st}<em class="st st-{st === '??' ? 'n' : st[0]}">{st === "??" ? "N" : st[0]}</em>{/if}
    </button>
  {/if}
{/each}

<style>
  .row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 5px;
    height: 24px;
    padding-right: 8px;
    font-size: var(--t-sm);
    text-align: left;
    color: var(--ink-2);
    border-radius: 5px;
    white-space: nowrap;
  }
  .row:hover {
    background: var(--paper-2);
  }
  .row.on {
    background: var(--accent-mist);
    color: var(--accent);
  }
  .row span,
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    flex: 1;
    min-width: 0;
  }
  .dir {
    color: var(--ink);
  }
  .ch {
    color: var(--warn-ink, oklch(0.55 0.13 70));
  }
  .st {
    font-style: normal;
    font-size: 0.66rem;
    font-weight: 700;
    flex: none;
  }
  .st-M,
  .st-R {
    color: oklch(0.6 0.13 70);
  }
  .st-A,
  .st-n {
    color: var(--ok);
  }
  .st-D {
    color: var(--danger);
  }
</style>
