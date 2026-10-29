<script lang="ts">
  // Terminale dello Studio: la shell dell'utente nella cartella del progetto (pseudo-terminale in Rust, disegno con xterm.js).
  import { onMount } from "svelte";
  import { Terminal } from "@xterm/xterm";
  import { FitAddon } from "@xterm/addon-fit";
  import "@xterm/xterm/css/xterm.css";
  import { api, errorText } from "../api";
  import { on } from "../events";

  let { project, active = true, onexit }: { project: string; active?: boolean; onexit?: (code: number | null) => void } = $props();
  let el: HTMLDivElement;
  let term: Terminal | undefined;
  let fit: FitAddon | undefined;

  /** I token del tema sono in OKLCH: xterm vuole RGB, quindi si fa disegnare il colore al browser e lo si rilegge */
  function rgb(css: string) {
    const c = document.createElement("canvas");
    c.width = c.height = 1;
    const ctx = c.getContext("2d", { willReadFrequently: true })!;
    ctx.fillStyle = css;
    ctx.fillRect(0, 0, 1, 1);
    const [r, g, b] = ctx.getImageData(0, 0, 1, 1).data;
    return `rgb(${r}, ${g}, ${b})`;
  }

  function tema() {
    const st = getComputedStyle(document.documentElement);
    const v = (n: string) => rgb(st.getPropertyValue(n).trim() || "black");
    const scuro = document.documentElement.dataset.theme === "dark";
    // Colori ANSI leggibili sul fondo dell'app, in chiaro e in scuro
    const ansi = scuro
      ? { black: "#3b4252", red: "#ff7b85", green: "#7ee2a8", yellow: "#f5d27a", blue: "#8fb2ff", magenta: "#d7a6ff", cyan: "#7fd8e8", white: "#d8dee9", brightBlack: "#6b7385", brightRed: "#ff9aa2", brightGreen: "#a0f0c0", brightYellow: "#ffe39c", brightBlue: "#b0c8ff", brightMagenta: "#e6c2ff", brightCyan: "#a5ecf5", brightWhite: "#ffffff" }
      : { black: "#2e3440", red: "#c4314b", green: "#1f8a4c", yellow: "#a06a00", blue: "#3456c4", magenta: "#8a3fb8", cyan: "#13809a", white: "#8a8fa0", brightBlack: "#5c6475", brightRed: "#d9455f", brightGreen: "#25a35a", brightYellow: "#b97c00", brightBlue: "#4a6ee0", brightMagenta: "#a352d1", brightCyan: "#1795b4", brightWhite: "#3b4252" };
    return {
      background: v("--paper"),
      foreground: v("--ink"),
      cursor: v("--accent"),
      cursorAccent: v("--paper"),
      selectionBackground: rgb(`color-mix(in oklch, ${st.getPropertyValue("--accent").trim() || "blue"} 28%, transparent)`).replace("rgb", "rgba").replace(")", ", 0.35)"),
      ...ansi,
    };
  }

  function adatta(id: string | null) {
    if (!term || !fit || !el?.offsetParent) return;
    fit.fit();
    if (id) api.termResize(id, term.cols, term.rows).catch(() => {});
  }

  let currentId: string | null = null;

  onMount(() => {
    term = new Terminal({
      fontFamily: 'ui-monospace, "SF Mono", Menlo, Consolas, monospace',
      fontSize: 12.5,
      lineHeight: 1.2,
      cursorBlink: true,
      scrollback: 5000,
      macOptionIsMeta: true,
      theme: tema(),
    });
    fit = new FitAddon();
    term.loadAddon(fit);
    term.open(el);
    fit.fit();

    const decodifica = (b64: string) => Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
    // L'uscita può arrivare prima che si sappia l'id (il prompt parte subito): si tiene da parte
    const primi: { id: string; data: string }[] = [];
    const stacchi: (() => void)[] = [];
    let chiuso = false;

    (async () => {
      stacchi.push(
        await on("term-data", (p) => {
          if (p.id === currentId) term?.write(decodifica(p.data));
          else if (!currentId) primi.push(p);
        }),
      );
      stacchi.push(
        await on("term-exit", (p) => {
          if (p.id !== currentId) return;
          term?.write(`\r\n\x1b[2m[sessione terminata${p.code ? `, codice ${p.code}` : ""}]\x1b[0m\r\n`);
          currentId = null;
          onexit?.(p.code);
        }),
      );
      const id = await api.termOpen(project, term!.cols, term!.rows);
      if (chiuso) {
        api.termClose(id).catch(() => {});
        return;
      }
      currentId = id;
      for (const p of primi.splice(0)) if (p.id === id) term?.write(decodifica(p.data));
    })().catch((e) => term?.write(`\x1b[31m${errorText(e)}\x1b[0m\r\n`));

    term.onData((d) => {
      if (currentId) api.termWrite(currentId, d).catch(() => {});
    });

    const ro = new ResizeObserver(() => adatta(currentId));
    ro.observe(el);
    // Cambio di tema (chiaro/scuro, accento): il terminale segue
    const mo = new MutationObserver(() => term && (term.options.theme = tema()));
    mo.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme", "style", "class"] });

    return () => {
      chiuso = true;
      ro.disconnect();
      mo.disconnect();
      stacchi.forEach((f) => f());
      if (currentId) api.termClose(currentId).catch(() => {});
      term?.dispose();
    };
  });

  // Quando la scheda torna visibile: dimensioni giuste e cursore pronto
  $effect(() => {
    if (active)
      requestAnimationFrame(() => {
        adatta(currentId);
        term?.focus();
      });
  });
</script>

<div class="term" bind:this={el}></div>

<style>
  .term {
    height: 100%;
    padding: 6px 4px 4px 10px;
    background: var(--paper);
  }
  .term :global(.xterm-viewport) {
    scrollbar-width: thin;
    scrollbar-color: color-mix(in oklch, var(--accent) 25%, transparent) transparent;
  }
</style>
