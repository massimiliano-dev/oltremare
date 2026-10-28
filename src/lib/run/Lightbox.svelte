<script lang="ts">
  import Icon from "../Icon.svelte";
  import { lightbox } from "../state/lightbox.svelte";
  import { fadeOut } from "../motion";

  function onkey(e: KeyboardEvent) {
    if (!lightbox.open) return;
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      lightbox.close();
    } else if (e.key === "ArrowRight") lightbox.move(1);
    else if (e.key === "ArrowLeft") lightbox.move(-1);
  }
  const cur = $derived(lightbox.images[lightbox.index]);
</script>

<svelte:window onkeydowncapture={onkey} />

{#if lightbox.open && cur}
  <div class="lb" role="dialog" aria-label="Screenshot" out:fadeOut>
    <button class="bg" onclick={() => lightbox.close()} aria-label="Chiudi"></button>
    <figure>
      <img src={cur.src} alt={cur.caption} />
      <figcaption>{cur.caption} · {lightbox.index + 1} di {lightbox.images.length}</figcaption>
    </figure>
    {#if lightbox.images.length > 1}
      <button class="nav prev" onclick={() => lightbox.move(-1)} aria-label="Precedente"><Icon name="left" size={20} /></button>
      <button class="nav next" onclick={() => lightbox.move(1)} aria-label="Successivo"><Icon name="chevron" size={20} /></button>
    {/if}
    <button class="close icon-btn" onclick={() => lightbox.close()} aria-label="Chiudi"><Icon name="x" /></button>
  </div>
{/if}

<style>
  .lb {
    position: absolute;
    inset: 0;
    z-index: 90;
    display: grid;
    place-items: center;
    animation: settle 0.2s;
  }
  .bg {
    position: absolute;
    inset: 0;
    background: oklch(0.12 0.03 var(--accent-h) / 0.82);
  }
  figure {
    position: relative;
    margin: 0;
    display: grid;
    gap: 10px;
    justify-items: center;
    max-width: 90vw;
    max-height: 88vh;
  }
  img {
    max-width: 90vw;
    max-height: 80vh;
    border-radius: var(--r-md);
    box-shadow: 0 20px 60px -20px black;
  }
  figcaption {
    color: oklch(0.92 0.02 var(--accent-h));
    font-size: var(--t-sm);
  }
  .nav {
    position: absolute;
    top: 50%;
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    margin-top: -22px;
    border-radius: 50%;
    color: white;
    background: oklch(1 0 0 / 0.12);
  }
  .nav:hover {
    background: oklch(1 0 0 / 0.22);
  }
  .prev {
    left: 24px;
  }
  .next {
    right: 24px;
  }
  .close {
    position: absolute;
    top: 50px;
    right: 24px;
    color: white;
  }
</style>
