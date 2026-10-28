<script lang="ts">
  import Icon from "../Icon.svelte";
  let { icon, hue, size = 28 }: { icon: string; hue: number; size?: number } = $props();
</script>

<span class="disc" style:--h={hue} style:width="{size}px" style:height="{size}px">
  <Icon name={icon} size={Math.round(size * 0.56)} />
</span>

<style>
  .disc {
    display: inline-grid;
    place-items: center;
    flex: none;
    border-radius: 32%;
    color: oklch(0.98 0.02 var(--h));
    background: linear-gradient(160deg, oklch(0.62 0.17 var(--h)), oklch(0.42 0.17 var(--h)));
    box-shadow: inset 0 1px 0 oklch(1 0 0 / 0.2), 0 1px 2px oklch(0.3 0.12 var(--h) / 0.3);
  }
</style>
