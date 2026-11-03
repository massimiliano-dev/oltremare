<script lang="ts">
  // Editor di codice (CodeMirror 6): evidenziazione per linguaggio, numeri di riga, ricerca, ⌘S per salvare.
  import { onMount } from "svelte";
  import type { EditorView } from "codemirror";
  import { theme } from "../state/theme.svelte";

  let {
    path,
    value,
    onchange,
    onsave,
    goto = null,
    oncursor,
    diagnostics = [],
    ondefinition,
  }: {
    path: string;
    value: string;
    onchange: (v: string) => void;
    onsave: () => void;
    goto?: { line: number; col: number; n: number } | null;
    oncursor?: (line: number, col: number, cursors: number) => void;
    diagnostics?: { line: number; col: number; severity: "error" | "warning"; message: string }[];
    ondefinition?: (name: string) => void;
  } = $props();
  let host: HTMLDivElement | undefined = $state();
  let view: EditorView | null = null;
  let applying = false;

  async function lingua(p: string) {
    const ext = p.split(".").pop()?.toLowerCase() ?? "";
    if (["ts", "tsx", "js", "jsx", "mjs", "cjs", "svelte", "vue"].includes(ext)) {
      const { javascript } = await import("@codemirror/lang-javascript");
      return javascript({ typescript: ext.startsWith("ts") || ext === "svelte" || ext === "vue", jsx: ext.endsWith("x") });
    }
    if (ext === "rs") return (await import("@codemirror/lang-rust")).rust();
    if (["html", "htm"].includes(ext)) return (await import("@codemirror/lang-html")).html();
    if (["css", "scss", "less"].includes(ext)) return (await import("@codemirror/lang-css")).css();
    if (["json", "jsonc"].includes(ext)) return (await import("@codemirror/lang-json")).json();
    if (["md", "mdx", "markdown"].includes(ext)) return (await import("@codemirror/lang-markdown")).markdown();
    if (ext === "py") return (await import("@codemirror/lang-python")).python();
    return null;
  }

  onMount(() => {
    let disposed = false;
    (async () => {
      const [{ EditorView, basicSetup }, { keymap }, { EditorState }, { oneDark }, { lintGutter }] = await Promise.all([
        import("codemirror"),
        import("@codemirror/view"),
        import("@codemirror/state"),
        import("@codemirror/theme-one-dark"),
        import("@codemirror/lint"),
      ]);
      // La parola sotto il cursore (o sotto il clic), per «vai alla definizione»
      const parola = (st: import("@codemirror/state").EditorState, pos: number) => {
        const w = st.wordAt(pos);
        return w ? st.sliceDoc(w.from, w.to) : "";
      };
      if (disposed || !host) return;
      const lang = await lingua(path);
      const chiaro = EditorView.theme({
        "&": { height: "100%", fontSize: "12.5px", backgroundColor: "var(--paper)" },
        ".cm-gutters": { backgroundColor: "var(--paper)", color: "var(--ink-3)", border: "none" },
        ".cm-activeLine, .cm-activeLineGutter": { backgroundColor: "var(--accent-mist)" },
        ".cm-scroller": { fontFamily: "ui-monospace, 'SF Mono', Menlo, monospace", lineHeight: "1.55" },
      });
      view = new EditorView({
        parent: host,
        state: EditorState.create({
          doc: value,
          extensions: [
            basicSetup,
            keymap.of([
              { key: "Mod-s", preventDefault: true, run: () => (onsave(), true) },
              { key: "F12", preventDefault: true, run: (v) => (ondefinition?.(parola(v.state, v.state.selection.main.head)), true) },
            ]),
            lintGutter(),
            // ⌘+⌥+clic: vai alla definizione (⌘+clic resta «aggiungi un cursore»)
            EditorView.domEventHandlers({
              mousedown: (e, v) => {
                if (!(e.metaKey && e.altKey) || !ondefinition) return false;
                const pos = v.posAtCoords({ x: e.clientX, y: e.clientY });
                if (pos == null) return false;
                e.preventDefault();
                ondefinition(parola(v.state, pos));
                return true;
              },
            }),
            chiaro,
            ...(theme.resolved === "dark" ? [oneDark] : []),
            ...(lang ? [lang] : []),
            EditorView.updateListener.of((u) => {
              if (u.docChanged && !applying) onchange(u.state.doc.toString());
              if (u.selectionSet || u.docChanged) {
                const h = u.state.selection.main.head;
                const l = u.state.doc.lineAt(h);
                oncursor?.(l.number, h - l.from + 1, u.state.selection.ranges.length);
              }
            }),
          ],
        }),
      });
      vai(goto);
      segnala(diagnostics);
    })();
    return () => {
      disposed = true;
      view?.destroy();
    };
  });

  // Salto a una riga (ricerca nel progetto): cursore lì, riga al centro
  async function vai(g: { line: number; col: number } | null) {
    if (!g || !view) return;
    const { EditorView: EV } = await import("@codemirror/view");
    const doc = view.state.doc;
    const l = doc.line(Math.min(Math.max(g.line, 1), doc.lines));
    const pos = Math.min(l.from + Math.max(g.col - 1, 0), l.to);
    view.dispatch({ selection: { anchor: pos }, effects: EV.scrollIntoView(pos, { y: "center" }) });
    view.focus();
  }
  $effect(() => {
    vai(goto);
  });

  // Errori e avvisi del compilatore, sottolineati nel testo
  async function segnala(d: typeof diagnostics) {
    if (!view) return;
    const { setDiagnostics } = await import("@codemirror/lint");
    const doc = view.state.doc;
    const lista = d
      .filter((x) => x.line >= 1 && x.line <= doc.lines)
      .map((x) => {
        const l = doc.line(x.line);
        const from = Math.min(l.from + Math.max(x.col - 1, 0), l.to);
        const w = view!.state.wordAt(from);
        return { from, to: w && w.from === from ? w.to : Math.min(from + 1, l.to), severity: x.severity, message: x.message };
      });
    view.dispatch(setDiagnostics(view.state, lista));
  }
  $effect(() => {
    segnala(diagnostics);
  });

  // Il contenuto cambiato da fuori (Claude ha modificato il file): si aggiorna l'editor
  $effect(() => {
    const v = value;
    if (view && v !== view.state.doc.toString()) {
      applying = true;
      view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: v } });
      applying = false;
    }
  });
</script>

<div class="cm" bind:this={host}></div>

<style>
  .cm {
    height: 100%;
    overflow: hidden;
  }
  .cm :global(.cm-editor) {
    height: 100%;
  }
  .cm :global(.cm-editor.cm-focused) {
    outline: none;
  }
</style>
