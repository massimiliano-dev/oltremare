<script lang="ts">
  // File e screenshot citati in un messaggio: anteprime delle immagini e schede dei file, cliccabili.
  import Icon from "../Icon.svelte";
  import { api, errorText } from "../api";
  import { ui } from "../state/ui.svelte";
  import { fileName, fileIcon, fileExt } from "./files";

  let { text, root = null, onopen }: { text: string; root?: string | null; onopen?: (path: string) => void } = $props();

  const IMG = ["png", "jpg", "jpeg", "gif", "webp", "bmp"];
  const EXT = "png|jpe?g|gif|webp|bmp|svg|pdf|md|txt|csv|json|ya?ml|toml|html?|css|scss|ts|tsx|js|jsx|mjs|svelte|vue|rs|py|go|java|kt|swift|cs|rb|php|sh|sql|xlsx?|docx?|pptx?|mp4|mov|zip";
  const RE = new RegExp(`(?:^|[\\s\`"'(\\[<:])((?:~\\/|\\/|\\.\\/)?(?:[\\w.@-]+\\/)+[\\w.@ -]*?\\.(?:${EXT})|(?:~\\/|\\/)[\\w.@-]+\\.(?:${EXT}))(?=$|[\\s\`"')\\]>:,;.])`, "gi");

  const paths = $derived.by(() => {
    const out: string[] = [];
    for (const m of text.matchAll(RE)) {
      let p = m[1].trim();
      if (p.startsWith("./")) p = p.slice(2);
      if (!p.startsWith("/") && !p.startsWith("~")) {
        if (!root) continue;
        p = `${root}/${p}`;
      }
      if (!out.includes(p)) out.push(p);
      if (out.length >= 8) break;
    }
    return out;
  });
  const isImg = (p: string) => IMG.includes(p.split(".").pop()?.toLowerCase() ?? "");

  let thumbs = $state<Record<string, string | "x">>({});
  let big = $state<{ path: string; src: string } | null>(null);

  $effect(() => {
    for (const p of paths.filter(isImg)) {
      if (thumbs[p]) continue;
      thumbs[p] = "x";
      api.imageThumb(p, 420).then((s) => (thumbs[p] = s)).catch(() => (thumbs[p] = "x"));
    }
  });

  async function openImage(p: string) {
    try {
      big = { path: p, src: await api.imageThumb(p, 2000) };
    } catch (e) {
      ui.toast(errorText(e), "error");
    }
  }

  function openFile(p: string) {
    if (onopen) onopen(p);
    else api.openPath(p).catch((e) => ui.toast(errorText(e), "error"));
  }
</script>

{#if paths.length}
  <div class="mentions">
    {#each paths as p (p)}
      {#if isImg(p) && thumbs[p] && thumbs[p] !== "x"}
        <button class="thumb" onclick={() => openImage(p)} title="{p} · clicca per ingrandire">
          <img src={thumbs[p]} alt={fileName(p)} />
          <span>{fileName(p)}</span>
        </button>
      {:else}
        <button class="file" onclick={() => openFile(p)} title={p}>
          <span class="ext"><Icon name={fileIcon(p)} size={14} /></span>
          <span class="fname">{fileName(p)}</span>
          <small>{fileExt(fileName(p))}</small>
        </button>
      {/if}
    {/each}
  </div>
{/if}

{#if big}
  <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
  <div class="box" onclick={() => (big = null)}>
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <figure onclick={(e) => e.stopPropagation()}>
      <img src={big.src} alt={fileName(big.path)} />
      <figcaption>
        <span>{big.path.replace(/^\/Users\/[^/]+/, "~")}</span>
        <button class="btn btn-quiet sm" onclick={() => api.openPath(big!.path).catch((e) => ui.toast(errorText(e), "error"))}><Icon name="external" size={13} /> Apri</button>
        <button class="icon-btn" onclick={() => (big = null)} aria-label="Chiudi"><Icon name="x" size={15} /></button>
      </figcaption>
    </figure>
  </div>
{/if}

<style>
  .mentions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 6px;
  }
  .thumb {
    display: grid;
    gap: 4px;
    max-width: 220px;
    padding: 4px;
    border-radius: 10px;
    background: var(--paper-2);
    box-shadow: inset 0 0 0 1px var(--rule-soft);
    text-align: left;
    transition: box-shadow 0.15s, transform 0.15s;
  }
  .thumb:hover {
    box-shadow: inset 0 0 0 1.5px var(--accent);
    transform: translateY(-1px);
  }
  .thumb img {
    display: block;
    max-width: 212px;
    max-height: 150px;
    border-radius: 7px;
    object-fit: cover;
  }
  .thumb span,
  .fname {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--t-xs);
    color: var(--ink-2);
  }
  .thumb span {
    padding: 0 4px 2px;
  }
  .file {
    display: flex;
    align-items: center;
    gap: 7px;
    max-width: 260px;
    height: 32px;
    padding: 0 10px 0 5px;
    border-radius: 9px;
    background: var(--paper-2);
    box-shadow: inset 0 0 0 1px var(--rule-soft);
    transition: box-shadow 0.15s;
  }
  .file:hover {
    box-shadow: inset 0 0 0 1.5px var(--accent);
  }
  .ext {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: 6px;
    color: var(--accent);
    background: color-mix(in oklch, var(--accent) 12%, transparent);
  }
  .file small {
    font-size: 0.64rem;
    font-weight: 700;
    letter-spacing: 0.04em;
    color: var(--ink-3);
  }
  .box {
    position: fixed;
    inset: 0;
    z-index: 80;
    display: grid;
    place-items: center;
    padding: 30px;
    background: oklch(0.15 0.02 275 / 0.72);
    backdrop-filter: blur(6px);
  }
  figure {
    margin: 0;
    max-width: 100%;
    max-height: 100%;
    display: grid;
    gap: 8px;
    padding: 10px;
    border-radius: 14px;
    background: var(--paper);
    box-shadow: var(--shadow-sheet);
  }
  figure img {
    max-width: calc(100vw - 100px);
    max-height: calc(100vh - 140px);
    border-radius: 8px;
    object-fit: contain;
  }
  figcaption {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
  figcaption span {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sm {
    height: 26px;
    padding: 0 9px;
    font-size: var(--t-xs);
    gap: 4px;
  }
</style>
