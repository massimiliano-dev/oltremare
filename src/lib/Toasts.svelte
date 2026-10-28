<script lang="ts">
  import { ui } from "./state/ui.svelte";
  import Icon from "./Icon.svelte";
  import { flip } from "svelte/animate";
  import { sheet } from "./motion";
</script>

<div class="toasts" aria-live="polite">
  {#each ui.toasts as t (t.id)}
    <div class="toast" class:error={t.kind === "error"} out:sheet={{ y: 10, scale: 0.97, duration: 160 }} animate:flip={{ duration: 220 }}>
      <span class="text">{t.text}</span>
      {#if t.action}
        <button
          class="action"
          onclick={() => {
            t.action!.run();
            ui.dismiss(t.id);
          }}>{t.action.label}</button
        >
      {/if}
      <button class="x" onclick={() => ui.dismiss(t.id)} aria-label="Chiudi"><Icon name="x" size={13} /></button>
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: absolute;
    z-index: 100;
    left: 50%;
    bottom: 22px;
    transform: translateX(-50%);
    display: grid;
    gap: 8px;
    justify-items: center;
    pointer-events: none;
  }
  .toast {
    display: flex;
    align-items: center;
    gap: 14px;
    max-width: min(560px, calc(100vw - 40px));
    min-height: 42px;
    padding: 6px 8px 6px 16px;
    border-radius: 12px;
    background: oklch(0.24 0.11 var(--accent-h));
    color: oklch(0.98 0.012 var(--accent-h));
    font-size: var(--t-sm);
    box-shadow: 0 14px 34px -12px oklch(0.15 0.1 var(--accent-h) / 0.6);
    pointer-events: auto;
    animation: up 0.3s var(--ease-out);
  }
  .toast.error {
    background: oklch(0.34 0.11 22);
  }
  @keyframes up {
    from {
      opacity: 0;
      transform: translateY(10px);
    }
  }
  .text {
    line-height: 1.35;
    user-select: text;
    -webkit-user-select: text;
  }
  .action {
    font-weight: 700;
    color: oklch(0.86 0.09 var(--accent-h));
  }
  .action:hover {
    color: white;
  }
  .x {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border-radius: 6px;
    color: oklch(0.75 0.06 var(--accent-h));
  }
  .x:hover {
    background: oklch(1 0 0 / 0.1);
    color: white;
  }
</style>
