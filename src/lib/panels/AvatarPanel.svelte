<script lang="ts">
  import Icon from "../Icon.svelte";
  import Avatar from "../avatar/Avatar.svelte";
  import { avatar } from "../state/avatar.svelte";
  import { settings } from "../state/settings.svelte";
  import { api } from "../api";
</script>

<section class="panel" aria-label="Oltremare">
  <div class="faces" role="radiogroup" aria-label="Volto">
    <button role="radio" aria-checked={avatar.face === "character"} class:on={avatar.face === "character"} onclick={() => settings.save({ avatarFace: "character" })} title="Personaggio"><Icon name="face" size={15} /></button>
    <button role="radio" aria-checked={avatar.face === "orb"} class:on={avatar.face === "orb"} onclick={() => settings.save({ avatarFace: "orb" })} title="Orb"><Icon name="orb" size={15} /></button>
  </div>
  <button class="pop" class:on={settings.s?.companion} onclick={() => api.companionSet(!settings.s?.companion)} title={settings.s?.companion ? "Nascondi il compagno" : "Tienimi sullo schermo (compagno sempre in primo piano)"}>
    <Icon name="external" size={14} />
  </button>
  <Avatar size={168} />
  <p class="label" aria-live="polite">{avatar.label}</p>
</section>

<style>
  .panel {
    position: relative;
    display: grid;
    justify-items: center;
    align-content: center;
    gap: 2px;
    padding: 18px 16px 14px;
    border-bottom: 1px solid var(--rule-soft);
    background: radial-gradient(120% 90% at 50% 40%, var(--accent-mist), transparent 70%);
  }
  .label {
    margin: 0;
    font-family: var(--font-serif);
    font-style: italic;
    font-size: 1.25rem;
    color: var(--ink-2);
  }
  .pop {
    position: absolute;
    top: 10px;
    left: 10px;
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: 8px;
    color: var(--ink-3);
  }
  .pop:hover {
    color: var(--accent);
    background: var(--paper-2);
  }
  .pop.on {
    color: var(--accent);
    background: var(--accent-mist);
  }
  .faces {
    position: absolute;
    top: 10px;
    right: 10px;
    display: flex;
    gap: 2px;
    padding: 2px;
    border-radius: 8px;
    background: var(--paper-2);
    box-shadow: inset 0 0 0 1px var(--rule-soft);
  }
  .faces button {
    display: grid;
    place-items: center;
    width: 26px;
    height: 24px;
    border-radius: 6px;
    color: var(--ink-3);
  }
  .faces button.on {
    color: var(--accent);
    background: var(--paper);
    box-shadow: 0 1px 2px oklch(0.3 0.1 var(--accent-h) / 0.12);
  }
</style>
