<script lang="ts" module>
  // Icone delle azioni di Claude, disegnate come quelle dei file: un colore per famiglia d'azione.
  export type ToolKind = "read" | "edit" | "write" | "bash" | "search" | "web" | "plan" | "agent" | "mcp" | "other";
  export function toolKind(name: string): ToolKind {
    const n = name.toLowerCase();
    if (n.includes("·") || n.includes("mcp")) return "mcp";
    if (["read", "notebookread", "ls"].includes(n)) return "read";
    if (["edit", "multiedit", "notebookedit"].includes(n)) return "edit";
    if (n === "write") return "write";
    if (["bash", "bashoutput", "killshell", "killbash"].includes(n)) return "bash";
    if (["grep", "glob", "toolsearch"].includes(n)) return "search";
    if (["webfetch", "websearch"].includes(n)) return "web";
    if (["exitplanmode", "todowrite", "enterplanmode", "askuserquestion"].includes(n)) return "plan";
    if (["task", "agent"].includes(n)) return "agent";
    return "other";
  }
  export const TOOL_HUE: Record<ToolKind, [number, number]> = {
    read: [245, 0.13],
    edit: [70, 0.15],
    write: [150, 0.14],
    bash: [165, 0.1],
    search: [200, 0.12],
    web: [340, 0.14],
    plan: [285, 0.15],
    agent: [305, 0.15],
    mcp: [35, 0.15],
    other: [262, 0.05],
  };
  export const TOOL_LABEL: Record<ToolKind, string> = {
    read: "Legge",
    edit: "Modifica",
    write: "Crea",
    bash: "Terminale",
    search: "Cerca",
    web: "Web",
    plan: "Piano",
    agent: "Sottoagente",
    mcp: "Gestionale",
    other: "Azione",
  };
</script>

<script lang="ts">
  let { kind, size = 22 }: { kind: ToolKind; size?: number } = $props();
  const hc = $derived(TOOL_HUE[kind]);
</script>

<span class="tg" style="--h:{hc[0]}; --c:{hc[1]}; width:{size}px; height:{size}px" aria-hidden="true">
  <svg width={size * 0.64} height={size * 0.64} viewBox="0 0 16 16">
    {#if kind === "read"}
      <path d="M1.8 8s2.3-4.2 6.2-4.2S14.2 8 14.2 8s-2.3 4.2-6.2 4.2S1.8 8 1.8 8z" /><circle cx="8" cy="8" r="1.9" />
    {:else if kind === "edit"}
      <path d="M10.6 2.6l2.8 2.8-7.6 7.6-3.4.6.6-3.4z" /><path d="M9.2 4l2.8 2.8" />
    {:else if kind === "write"}
      <path d="M4 1.8h5l3 3V13c0 .7-.5 1.2-1.2 1.2H4c-.7 0-1.2-.5-1.2-1.2V3c0-.7.5-1.2 1.2-1.2z" /><path d="M7.4 7.4v4.2M5.3 9.5h4.2" />
    {:else if kind === "bash"}
      <path d="m3 4.6 3.2 3.2L3 11" /><path d="M8 11.4h5" />
    {:else if kind === "search"}
      <circle cx="7" cy="7" r="4.2" /><path d="m10.2 10.2 3.4 3.4" />
    {:else if kind === "web"}
      <circle cx="8" cy="8" r="6" /><path d="M2 8h12M8 2c1.7 1.8 2.6 3.8 2.6 6S9.7 12.2 8 14C6.3 12.2 5.4 10.2 5.4 8S6.3 3.8 8 2z" />
    {:else if kind === "plan"}
      <path d="M6.2 4h7M6.2 8h7M6.2 12h7" /><path d="m2.4 4 .8.8 1.4-1.6M2.4 8l.8.8 1.4-1.6M2.6 12h1.6" />
    {:else if kind === "agent"}
      <path d="M8 1.8l1.4 3.6 3.8.3-2.9 2.5.9 3.8L8 10l-3.2 2 .9-3.8-2.9-2.5 3.8-.3z" />
    {:else if kind === "mcp"}
      <path d="M5.6 2v3.2M10.4 2v3.2M3.6 5.2h8.8v2.4a4.4 4.4 0 0 1-8.8 0z" /><path d="M8 12v2.2" />
    {:else}
      <circle cx="8" cy="8" r="2.2" /><path d="M8 2.2v1.6M8 12.2v1.6M2.2 8h1.6M12.2 8h1.6" />
    {/if}
  </svg>
</span>

<style>
  .tg {
    flex: none;
    display: grid;
    place-items: center;
    border-radius: 7px;
    background: oklch(0.72 var(--c) var(--h) / 0.18);
  }
  svg {
    overflow: visible;
  }
  path,
  circle {
    fill: none;
    stroke: oklch(0.5 calc(var(--c) * 1.05) var(--h));
    stroke-width: 1.5;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  :global([data-theme="dark"]) .tg {
    background: oklch(0.6 var(--c) var(--h) / 0.22);
  }
  :global([data-theme="dark"]) path,
  :global([data-theme="dark"]) circle {
    stroke: oklch(0.82 var(--c) var(--h));
  }
</style>
