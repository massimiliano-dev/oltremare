<script lang="ts" module>
  // Icone dei file e delle cartelle disegnate a mano (tratto 1.4 su griglia 16), colorate per tipo e per ruolo.
  type Kind = "code" | "markup" | "style" | "data" | "doc" | "config" | "image" | "test" | "lock" | "shell" | "file";
  const EXT: Record<string, [Kind, number, number]> = {
    ts: ["code", 250, 0.15], tsx: ["code", 250, 0.15], mts: ["code", 250, 0.15], cts: ["code", 250, 0.15],
    js: ["code", 92, 0.15], jsx: ["code", 92, 0.15], mjs: ["code", 92, 0.15], cjs: ["code", 92, 0.15],
    svelte: ["code", 35, 0.19], vue: ["code", 155, 0.15], rs: ["code", 48, 0.15], go: ["code", 205, 0.12],
    py: ["code", 235, 0.12], rb: ["code", 20, 0.17], php: ["code", 280, 0.12], java: ["code", 30, 0.14], kt: ["code", 300, 0.16],
    cs: ["code", 290, 0.15], swift: ["code", 40, 0.18], dart: ["code", 215, 0.13], c: ["code", 240, 0.08], cpp: ["code", 240, 0.11], h: ["code", 240, 0.08],
    html: ["markup", 38, 0.17], htm: ["markup", 38, 0.17], xml: ["markup", 38, 0.12], svg: ["image", 70, 0.15],
    css: ["style", 300, 0.15], scss: ["style", 340, 0.15], less: ["style", 270, 0.12],
    json: ["data", 78, 0.13], jsonc: ["data", 78, 0.13], csv: ["data", 150, 0.12], sql: ["data", 195, 0.12], prisma: ["data", 255, 0.1],
    md: ["doc", 250, 0.06], mdx: ["doc", 250, 0.09], txt: ["doc", 250, 0.04], pdf: ["doc", 25, 0.17],
    yaml: ["config", 160, 0.08], yml: ["config", 160, 0.08], toml: ["config", 160, 0.08], ini: ["config", 160, 0.06], env: ["config", 130, 0.12], conf: ["config", 160, 0.06],
    png: ["image", 340, 0.15], jpg: ["image", 340, 0.15], jpeg: ["image", 340, 0.15], gif: ["image", 340, 0.15], webp: ["image", 340, 0.15], ico: ["image", 340, 0.12], icns: ["image", 340, 0.12],
    lock: ["lock", 250, 0.03], sh: ["shell", 140, 0.12], zsh: ["shell", 140, 0.12], ps1: ["shell", 230, 0.12],
  };
  const NAMES: Record<string, [Kind, number, number]> = {
    "package.json": ["data", 25, 0.16], "cargo.toml": ["config", 48, 0.15], dockerfile: ["config", 225, 0.14], "docker-compose.yml": ["config", 225, 0.14],
    ".gitignore": ["config", 25, 0.1], ".env": ["config", 130, 0.12], "readme.md": ["doc", 230, 0.13], license: ["doc", 80, 0.1], makefile: ["shell", 25, 0.1],
  };

  export function fileStyle(name: string): { kind: Kind; hue: number; chroma: number } {
    const n = name.toLowerCase();
    if (NAMES[n]) return { kind: NAMES[n][0], hue: NAMES[n][1], chroma: NAMES[n][2] };
    if (/\.(test|spec)\.[a-z]+$/.test(n)) return { kind: "test", hue: 145, chroma: 0.15 };
    if (n.endsWith(".lock") || n === "package-lock.json" || n === "pnpm-lock.yaml") return { kind: "lock", hue: 250, chroma: 0.03 };
    if (n.startsWith(".env")) return { kind: "config", hue: 130, chroma: 0.12 };
    const ext = n.includes(".") ? n.split(".").pop()! : "";
    const e = EXT[ext];
    return e ? { kind: e[0], hue: e[1], chroma: e[2] } : { kind: "file", hue: 250, chroma: 0.03 };
  }

  const ROLES: [RegExp, number, number][] = [
    [/^(src|source|lib|core)$/, 275, 0.17],
    [/^(components?|ui|widgets|views)$/, 300, 0.15],
    [/^(app|pages|routes|screens)$/, 240, 0.15],
    [/^(public|static|assets|images|img|icons|media|fonts)$/, 340, 0.14],
    [/^(tests?|__tests__|spec|e2e|cypress)$/, 145, 0.15],
    [/^(docs?|documentation|guides)$/, 190, 0.12],
    [/^(scripts?|bin|tools)$/, 78, 0.14],
    [/^(api|server|backend|services?|functions)$/, 40, 0.16],
    [/^(styles?|css|theme)$/, 310, 0.14],
    [/^(state|stores?|hooks|utils?|helpers)$/, 210, 0.12],
    [/^(src-tauri|android|ios|native)$/, 48, 0.14],
    [/^(\.github|\.vscode|config|\.cargo|prisma|migrations|db)$/, 255, 0.06],
  ];

  export function folderStyle(name: string): { hue: number; chroma: number } {
    const n = name.toLowerCase();
    for (const [re, h, c] of ROLES) if (re.test(n)) return { hue: h, chroma: c };
    return { hue: 262, chroma: 0.07 };
  }
</script>

<script lang="ts">
  let { name, folder = false, open = false, size = 16 }: { name: string; folder?: boolean; open?: boolean; size?: number } = $props();
  const f = $derived(folder ? null : fileStyle(name));
  const d = $derived(folder ? folderStyle(name) : null);
  const hue = $derived(f?.hue ?? d?.hue ?? 262);
  const chroma = $derived(f?.chroma ?? d?.chroma ?? 0.07);
</script>

<svg class="fi" width={size} height={size} viewBox="0 0 16 16" aria-hidden="true" style="--h:{hue}; --c:{chroma}">
  {#if folder}
    {#if open}
      <path class="back" d="M1.6 4.2c0-.7.5-1.2 1.2-1.2h3.1l1.4 1.5h5.6c.7 0 1.2.5 1.2 1.2v.9" />
      <path class="front" d="M1.4 7.4c.1-.6.6-1 1.2-1h11.3c.8 0 1.3.7 1.1 1.4l-1.1 4.6c-.1.5-.6.9-1.2.9H2.9c-.7 0-1.2-.5-1.2-1.2z" />
    {:else}
      <path class="front" d="M1.6 4.2c0-.7.5-1.2 1.2-1.2h3.1l1.4 1.5h5.9c.7 0 1.2.5 1.2 1.2v6.5c0 .7-.5 1.2-1.2 1.2H2.8c-.7 0-1.2-.5-1.2-1.2z" />
      <path class="tab" d="M1.6 6.2h12.8" />
    {/if}
  {:else}
    <path class="doc" d="M3.6 1.6h5.6l3.2 3.2v8.4c0 .7-.5 1.2-1.2 1.2H3.6c-.7 0-1.2-.5-1.2-1.2V2.8c0-.7.5-1.2 1.2-1.2z" />
    <path class="fold" d="M9.2 1.6v2.6c0 .3.3.6.6.6h2.6" />
    {#if f?.kind === "code"}
      <path class="mark" d="M6.1 8.2 4.8 9.6l1.3 1.4M8.9 8.2l1.3 1.4-1.3 1.4" />
    {:else if f?.kind === "markup"}
      <path class="mark" d="M5.9 8.1 4.6 9.6l1.3 1.5M9.1 8.1l1.3 1.5-1.3 1.5M8 7.9l-1 3.4" />
    {:else if f?.kind === "style"}
      <path class="mark" d="M5 11.4c1.2 0 1.6-.8 1.6-1.5 0-.6-.5-1-1-1-.6 0-1 .4-1 1 0 .7-.3 1-.8 1.2M7 9.6l3-2.8" />
    {:else if f?.kind === "data"}
      <path class="mark" d="M6 7.6c-.7 0-.9.4-.9 1v.6c0 .4-.3.6-.6.6.3 0 .6.2.6.6v.6c0 .6.2 1 .9 1M9 7.6c.7 0 .9.4.9 1v.6c0 .4.3.6.6.6-.3 0-.6.2-.6.6v.6c0 .6-.2 1-.9 1" />
    {:else if f?.kind === "doc"}
      <path class="mark" d="M4.9 8h5.2M4.9 9.8h5.2M4.9 11.6h3.2" />
    {:else if f?.kind === "config"}
      <circle class="mark" cx="7.5" cy="9.8" r="1.3" /><path class="mark" d="M7.5 7.3v.8M7.5 11.5v.8M5 9.8h.8M9.2 9.8h.8" />
    {:else if f?.kind === "image"}
      <path class="mark" d="m4.6 11.8 2-2.4 1.4 1.5 1-1 1.6 1.9" /><circle class="mark" cx="9.2" cy="7.9" r=".7" />
    {:else if f?.kind === "test"}
      <path class="mark" d="M6.4 7.4v1.8l-1.6 2.6c-.2.4.1.8.5.8h4.4c.4 0 .7-.4.5-.8L8.6 9.2V7.4M6 7.4h3" />
    {:else if f?.kind === "lock"}
      <rect class="mark" x="5.3" y="9.1" width="4.4" height="3" rx=".6" /><path class="mark" d="M6.2 9.1V8.2a1.3 1.3 0 0 1 2.6 0v.9" />
    {:else if f?.kind === "shell"}
      <path class="mark" d="m5 8.4 1.5 1.3L5 11M7.6 11.2h2.6" />
    {/if}
  {/if}
</svg>

<style>
  .fi {
    flex: none;
    display: block;
    overflow: visible;
  }
  path,
  circle,
  rect {
    fill: none;
    stroke-width: 1.4;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .doc {
    stroke: oklch(0.6 var(--c) var(--h));
    fill: oklch(0.6 var(--c) var(--h) / 0.13);
  }
  .fold {
    stroke: oklch(0.6 var(--c) var(--h));
  }
  .mark {
    stroke: oklch(0.52 calc(var(--c) * 1.1) var(--h));
    stroke-width: 1.3;
  }
  .front {
    stroke: oklch(0.58 var(--c) var(--h));
    fill: oklch(0.72 var(--c) var(--h) / 0.32);
  }
  .back {
    stroke: oklch(0.58 var(--c) var(--h));
    fill: oklch(0.72 var(--c) var(--h) / 0.16);
  }
  .tab {
    stroke: oklch(0.58 var(--c) var(--h) / 0.55);
  }
  :global([data-theme="dark"]) .doc,
  :global([data-theme="dark"]) .fold {
    stroke: oklch(0.72 var(--c) var(--h));
  }
  :global([data-theme="dark"]) .mark {
    stroke: oklch(0.8 var(--c) var(--h));
  }
  :global([data-theme="dark"]) .front,
  :global([data-theme="dark"]) .back {
    stroke: oklch(0.74 var(--c) var(--h));
    fill: oklch(0.6 var(--c) var(--h) / 0.32);
  }
</style>
