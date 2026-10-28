<script lang="ts">
  import Icon from "../Icon.svelte";
  import { voice } from "../state/voice.svelte";
  import { avatar } from "../state/avatar.svelte";
  let { size = 30 }: { size?: number } = $props();
</script>

<button
  class="mic"
  class:on={voice.recording}
  style:--lv={voice.recording ? avatar.audio : 0}
  style:width="{size}px"
  style:height="{size}px"
  onclick={() => voice.toggle()}
  title={voice.recording ? "Sto ascoltando · clic per annullare" : "Parla (o di' «Ehi Oltremare»)"}
  aria-label={voice.recording ? "Annulla l'ascolto" : "Parla"}
  aria-pressed={voice.recording}
>
  <Icon name="mic" size={Math.round(size * 0.56)} />
</button>

<style>
  .mic {
    position: relative;
    display: grid;
    place-items: center;
    border-radius: 50%;
    flex: none;
    color: var(--ink-3);
    transition: color 0.15s, background 0.15s;
  }
  .mic:hover {
    color: var(--accent);
    background: var(--accent-mist);
  }
  .mic.on {
    color: var(--on-accent);
    background: var(--accent);
  }
  .mic.on::before {
    content: "";
    position: absolute;
    inset: -4px;
    border-radius: 50%;
    border: 2px solid var(--accent);
    opacity: calc(0.35 + var(--lv) * 0.65);
    transform: scale(calc(1 + var(--lv) * 0.35));
    transition: transform 0.08s linear, opacity 0.08s linear;
  }
</style>
