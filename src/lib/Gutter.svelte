<script lang="ts">
  // Bordo trascinabile tra due pannelli: mouse, tastiera (frecce) e doppio clic per ripristinare.
  import { layout, type PanelKind } from "./state/layout.svelte";

  let { left, right }: { left: PanelKind; right: PanelKind } = $props();
  // Si ridimensiona il pannello a larghezza fissa (la chat è elastica)
  const target = $derived<PanelKind>(left !== "chat" ? left : right);
  const sign = $derived(left !== "chat" ? 1 : -1);
  let dragging = $state(false);

  function down(e: PointerEvent) {
    e.preventDefault();
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
    dragging = true;
    const x0 = e.clientX;
    const w0 = layout.width(target);
    const move = (ev: PointerEvent) => layout.resize(target, w0 + sign * (ev.clientX - x0));
    const up = () => {
      dragging = false;
      el.removeEventListener("pointermove", move);
      el.removeEventListener("pointerup", up);
      el.removeEventListener("pointercancel", up);
    };
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerup", up);
    el.addEventListener("pointercancel", up);
  }

  function key(e: KeyboardEvent) {
    const step = e.shiftKey ? 48 : 16;
    if (e.key === "ArrowLeft") layout.resize(target, layout.width(target) - sign * step);
    else if (e.key === "ArrowRight") layout.resize(target, layout.width(target) + sign * step);
    else if (e.key === "Enter") layout.reset(target);
    else return;
    e.preventDefault();
  }
</script>

<!-- Separatore ridimensionabile (pattern ARIA «window splitter»): è focalizzabile e risponde alle frecce -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div
  class="gutter"
  class:dragging
  role="separator"
  aria-orientation="vertical"
  aria-label="Ridimensiona il pannello"
  aria-valuenow={layout.width(target)}
  tabindex="0"
  onpointerdown={down}
  ondblclick={() => layout.reset(target)}
  onkeydown={key}
></div>

<style>
  .gutter {
    position: relative;
    width: 1px;
    margin: 0 -3px;
    padding: 0 3px;
    box-sizing: content-box;
    z-index: 5;
    cursor: col-resize;
    background: var(--rule-soft);
    background-clip: content-box;
    touch-action: none;
    outline: none;
    transition: background-color 0.15s;
  }
  .gutter:hover,
  .gutter.dragging,
  .gutter:focus-visible {
    background-color: var(--accent);
  }
</style>
