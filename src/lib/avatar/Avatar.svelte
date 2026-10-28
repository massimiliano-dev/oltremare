<script lang="ts">
  import type { AvatarState } from "../api";
  import { avatar } from "../state/avatar.svelte";
  import Personaggio from "./Personaggio.svelte";
  import Orb from "./Orb.svelte";

  let {
    size = 200,
    state,
    level,
    face,
    track = true,
  }: { size?: number; state?: AvatarState; level?: number; face?: "character" | "orb"; track?: boolean } = $props();

  const s = $derived(state ?? avatar.shown);
  const l = $derived(level ?? avatar.level);
  const f = $derived(face ?? avatar.face);
</script>

<div class="avatar" style:width="{size}px" style:height="{size}px" role="img" aria-label="Oltremare: {avatar.label}">
  {#if f === "orb"}
    <Orb mood={s} level={l} {size} />
  {:else}
    <Personaggio mood={s} level={l} {size} {track} />
  {/if}
</div>

<style>
  .avatar {
    display: grid;
    place-items: center;
    flex: none;
  }
</style>
