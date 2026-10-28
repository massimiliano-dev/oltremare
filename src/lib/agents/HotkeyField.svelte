<script lang="ts">
  // Registra una scorciatoia premendo i tasti. Restituisce il formato degli acceleratori Tauri
  // (es. "Alt+Shift+Space") e la mostra con i simboli del sistema (⌥⇧Spazio).
  let { value = $bindable(""), onchange }: { value?: string; onchange?: (v: string) => void } = $props();
  let recording = $state(false);
  const mac = navigator.userAgent.includes("Mac");

  const SYMBOL: Record<string, string> = mac
    ? { CommandOrControl: "⌘", Command: "⌘", Control: "⌃", Alt: "⌥", Shift: "⇧", Space: "Spazio", Escape: "Esc", Enter: "↵" }
    : { CommandOrControl: "Ctrl", Control: "Ctrl", Alt: "Alt", Shift: "Maiusc", Space: "Spazio", Escape: "Esc", Enter: "Invio" };

  export function label(v: string) {
    return v ? v.split("+").map((k) => SYMBOL[k] ?? k).join(mac ? "" : "+") : "Nessuna";
  }

  function key(e: KeyboardEvent): string | null {
    if (["Meta", "Control", "Alt", "Shift"].includes(e.key)) return null;
    if (e.code.startsWith("Key")) return e.code.slice(3);
    if (e.code.startsWith("Digit")) return e.code.slice(5);
    if (/^F\d+$/.test(e.code)) return e.code;
    return ({ Space: "Space", Escape: "Escape", Enter: "Enter", Period: "Period", Comma: "Comma", Slash: "Slash" } as Record<string, string>)[e.code] ?? null;
  }

  function onkey(e: KeyboardEvent) {
    if (!recording) return;
    e.preventDefault();
    e.stopPropagation();
    if (e.key === "Escape" && !e.metaKey && !e.ctrlKey && !e.altKey && !e.shiftKey) {
      recording = false;
      return;
    }
    if (e.key === "Backspace" && !e.metaKey && !e.ctrlKey && !e.altKey) {
      value = "";
      recording = false;
      onchange?.("");
      return;
    }
    const k = key(e);
    if (!k) return;
    const mods = [e.metaKey ? (mac ? "Command" : "Super") : "", e.ctrlKey ? "Control" : "", e.altKey ? "Alt" : "", e.shiftKey ? "Shift" : ""].filter(Boolean);
    if (!mods.length && !/^F\d+$/.test(k)) return; // serve almeno un modificatore
    value = [...mods, k].join("+");
    recording = false;
    onchange?.(value);
  }
</script>

<svelte:window onkeydown={onkey} />

<button class="hk" class:rec={recording} onclick={() => (recording = !recording)}>
  {#if recording}Premi la combinazione… <small>Esc annulla · ⌫ toglie</small>{:else}<kbd>{label(value)}</kbd> <small>Cambia</small>{/if}
</button>

<style>
  .hk {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 36px;
    padding: 0 12px;
    border-radius: var(--r-sm);
    box-shadow: inset 0 0 0 1px var(--rule);
    background: var(--paper);
    font-size: var(--t-sm);
  }
  .hk.rec {
    box-shadow: inset 0 0 0 2px var(--accent);
    color: var(--accent);
    font-weight: 600;
  }
  .hk kbd {
    font-size: 0.8rem;
    min-width: auto;
    padding: 0 8px;
    height: 24px;
  }
  small {
    font-size: var(--t-xs);
    color: var(--ink-3);
    font-weight: 500;
  }
</style>
