<script lang="ts">
  // Il compagno: l'avatar in una finestrella trasparente sempre in primo piano.
  // Clic = parla · trascina = sposta · doppio clic = apre Oltremare.
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import Avatar from "./lib/avatar/Avatar.svelte";
  import { avatar } from "./lib/state/avatar.svelte";
  import { settings } from "./lib/state/settings.svelte";
  import { voice } from "./lib/state/voice.svelte";
  import { approvals } from "./lib/state/approvals.svelte";
  import { api } from "./lib/api";

  let start: { x: number; y: number } | null = null;
  let clickTimer: ReturnType<typeof setTimeout> | null = null;

  onMount(async () => {
    document.documentElement.classList.add("companion");
    await Promise.all([settings.load(), avatar.listen(), voice.init(), approvals.listen()]);
  });

  function down(e: PointerEvent) {
    start = { x: e.screenX, y: e.screenY };
  }
  function move(e: PointerEvent) {
    if (start && Math.hypot(e.screenX - start.x, e.screenY - start.y) > 4) {
      start = null;
      getCurrentWindow().startDragging().catch(() => {});
    }
  }
  function up() {
    if (!start) return;
    start = null;
    // Un clic parla; un doppio clic apre la finestra principale
    if (clickTimer) {
      clearTimeout(clickTimer);
      clickTimer = null;
      api.mainShow();
      return;
    }
    clickTimer = setTimeout(() => {
      clickTimer = null;
      voice.toggle();
    }, 260);
  }

  const bubble = $derived(
    approvals.current ? "Ho bisogno di te" : voice.recording ? "Ti ascolto…" : voice.transcript ? `«${voice.transcript}»` : avatar.shown === "idle" || avatar.shown === "sleep" ? "" : avatar.label,
  );
</script>

<div class="wrap" role="button" tabindex="0" aria-label="Oltremare: clic per parlare" onpointerdown={down} onpointermove={move} onpointerup={up}>
  <Avatar size={128} />
  {#if bubble}<p class="bubble" class:warn={!!approvals.current}>{bubble}</p>{/if}
</div>

<style>
  :global(html.companion),
  :global(html.companion body),
  :global(html.companion #app) {
    background: transparent !important;
  }
  .wrap {
    height: 100vh;
    display: grid;
    justify-items: center;
    align-content: start;
    padding-top: 4px;
    cursor: grab;
    user-select: none;
    -webkit-user-select: none;
  }
  .bubble {
    max-width: 144px;
    margin: -6px 0 0;
    padding: 4px 10px;
    border-radius: 999px;
    font-size: 0.72rem;
    font-weight: 600;
    text-align: center;
    color: var(--ink);
    background: var(--paper);
    box-shadow: 0 4px 14px -6px oklch(0.2 0.1 var(--accent-h) / 0.5), 0 0 0 1px var(--rule-soft);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .bubble.warn {
    color: var(--warn-ink);
  }
</style>
