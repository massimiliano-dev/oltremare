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
    oncursor?: (line: number, col: number, cursors: number, selection: { text: string; from: number; to: number } | null) => void;
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
      const [{ EditorView, basicSetup }, { keymap }, { EditorState }, { lintGutter }, { HighlightStyle, syntaxHighlighting }, { tags: t }] = await Promise.all([
        import("codemirror"),
        import("@codemirror/view"),
        import("@codemirror/state"),
        import("@codemirror/lint"),
        import("@codemirror/language"),
        import("@lezer/highlight"),
      ]);
      // Colori della sintassi dai token dell'app (variabili CSS: valgono in chiaro e in scuro)
      const sintassi = HighlightStyle.define([
        { tag: [t.keyword, t.controlKeyword, t.moduleKeyword, t.operatorKeyword, t.definitionKeyword], color: "var(--syn-kw)", fontWeight: "550" },
        { tag: [t.string, t.special(t.string), t.regexp], color: "var(--syn-str)" },
        { tag: [t.number, t.bool, t.null, t.atom], color: "var(--syn-num)" },
        { tag: [t.comment, t.lineComment, t.blockComment, t.docComment], color: "var(--syn-com)", fontStyle: "italic" },
        { tag: [t.function(t.variableName), t.function(t.propertyName), t.macroName], color: "var(--syn-fn)" },
        { tag: [t.typeName, t.className, t.namespace, t.tagName], color: "var(--syn-type)" },
        { tag: [t.propertyName, t.attributeName], color: "var(--syn-prop)" },
        { tag: [t.definition(t.variableName)], color: "var(--syn-def)" },
        { tag: [t.operator, t.punctuation, t.bracket, t.separator], color: "var(--syn-punct)" },
        { tag: [t.heading], color: "var(--syn-kw)", fontWeight: "700" },
        { tag: [t.link, t.url], color: "var(--syn-str)", textDecoration: "underline" },
        { tag: [t.meta, t.annotation], color: "var(--syn-com)" },
        { tag: t.invalid, color: "var(--danger)" },
      ]);
      // La parola sotto il cursore (o sotto il clic), per «vai alla definizione»
      const parola = (st: import("@codemirror/state").EditorState, pos: number) => {
        const w = st.wordAt(pos);
        return w ? st.sliceDoc(w.from, w.to) : "";
      };
      if (disposed || !host) return;
      const lang = await lingua(path);
      const aspetto = EditorView.theme({
        "&": { height: "100%", fontSize: "13px", backgroundColor: "var(--paper)", color: "var(--ink)" },
        ".cm-scroller": { fontFamily: "'SF Mono', ui-monospace, Menlo, monospace", lineHeight: "1.65", fontVariantLigatures: "contextual" },
        ".cm-content": { padding: "10px 0 40vh", caretColor: "var(--accent)" },
        ".cm-line": { padding: "0 18px 0 4px" },
        ".cm-gutters": { backgroundColor: "var(--paper)", color: "color-mix(in oklch, var(--ink-3) 70%, transparent)", border: "none", paddingLeft: "4px" },
        ".cm-lineNumbers .cm-gutterElement": { padding: "0 6px 0 6px", minWidth: "32px", fontSize: "11.5px" },
        ".cm-gutter-lint": { width: "12px" },
        ".cm-gutter-lint .cm-gutterElement": { padding: "0 2px" },
        ".cm-activeLineGutter": { backgroundColor: "transparent", color: "var(--accent)", fontWeight: "650" },
        ".cm-activeLine": { backgroundColor: "color-mix(in oklch, var(--accent) 5%, transparent)" },
        ".cm-cursor, .cm-dropCursor": { borderLeftColor: "var(--accent)", borderLeftWidth: "2px" },
        "&.cm-focused .cm-selectionBackground, .cm-selectionBackground, ::selection": { backgroundColor: "color-mix(in oklch, var(--accent) 22%, transparent) !important" },
        ".cm-selectionMatch": { backgroundColor: "color-mix(in oklch, var(--accent) 12%, transparent)" },
        "&.cm-focused .cm-matchingBracket": { backgroundColor: "color-mix(in oklch, var(--accent) 18%, transparent)", outline: "none" },
        ".cm-foldGutter .cm-gutterElement": { color: "var(--ink-3)", padding: "0 2px", opacity: "0.6" },
        ".cm-foldPlaceholder": { backgroundColor: "var(--paper-3)", border: "none", color: "var(--ink-2)", borderRadius: "4px", padding: "0 6px" },
        ".cm-panels": { backgroundColor: "var(--paper-2)", color: "var(--ink)", borderColor: "var(--rule-soft)" },
        ".cm-panels.cm-panels-top": { borderBottom: "1px solid var(--rule-soft)" },
        ".cm-panel.cm-search": { padding: "8px 12px", fontSize: "12px" },
        ".cm-panel.cm-search input, .cm-panel.cm-search button": { borderRadius: "6px", fontSize: "12px" },
        ".cm-textfield": { backgroundColor: "var(--paper)", border: "1px solid var(--rule)", color: "var(--ink)", padding: "3px 6px" },
        ".cm-button": { backgroundImage: "none", backgroundColor: "var(--paper)", border: "1px solid var(--rule)", color: "var(--ink-2)" },
        ".cm-searchMatch": { backgroundColor: "color-mix(in oklch, var(--warn) 30%, transparent)", outline: "none" },
        ".cm-searchMatch-selected": { backgroundColor: "color-mix(in oklch, var(--warn) 55%, transparent)" },
        ".cm-tooltip": { backgroundColor: "var(--paper)", border: "1px solid var(--rule)", borderRadius: "8px", boxShadow: "0 8px 24px -10px oklch(0.2 0.05 270 / 0.35)" },
        ".cm-tooltip-autocomplete > ul > li[aria-selected]": { backgroundColor: "color-mix(in oklch, var(--accent) 16%, transparent)", color: "var(--ink)" },
        ".cm-diagnostic": { padding: "6px 10px", fontSize: "12px", borderLeftWidth: "3px" },
        ".cm-lint-marker": { width: "0.8em", height: "0.8em" },
      }, { dark: theme.resolved === "dark" });
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
            aspetto,
            syntaxHighlighting(sintassi),
            ...(lang ? [lang] : []),
            EditorView.updateListener.of((u) => {
              if (u.docChanged && !applying) onchange(u.state.doc.toString());
              if (u.selectionSet || u.docChanged) {
                const h = u.state.selection.main.head;
                const l = u.state.doc.lineAt(h);
                const m = u.state.selection.main;
                const sel = m.empty ? null : { text: u.state.sliceDoc(m.from, m.to).slice(0, 4000), from: u.state.doc.lineAt(m.from).number, to: u.state.doc.lineAt(m.to).number };
                oncursor?.(l.number, h - l.from + 1, u.state.selection.ranges.length, sel);
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
    /* Palette della sintassi: toni dell'app, leggibili sul fondo carta */
    --syn-kw: oklch(0.5 0.17 300);
    --syn-str: oklch(0.52 0.13 150);
    --syn-num: oklch(0.55 0.15 55);
    --syn-com: oklch(0.6 0.03 260);
    --syn-fn: oklch(0.5 0.15 255);
    --syn-type: oklch(0.52 0.13 200);
    --syn-prop: oklch(0.48 0.09 230);
    --syn-def: oklch(0.38 0.06 260);
    --syn-punct: oklch(0.55 0.03 260);
  }
  :global([data-theme="dark"]) .cm {
    --syn-kw: oklch(0.78 0.13 300);
    --syn-str: oklch(0.8 0.12 150);
    --syn-num: oklch(0.82 0.12 60);
    --syn-com: oklch(0.62 0.03 260);
    --syn-fn: oklch(0.8 0.11 250);
    --syn-type: oklch(0.82 0.1 195);
    --syn-prop: oklch(0.82 0.06 230);
    --syn-def: oklch(0.9 0.03 260);
    --syn-punct: oklch(0.7 0.03 260);
  }
  .cm :global(.cm-editor) {
    height: 100%;
  }
  .cm :global(.cm-editor.cm-focused) {
    outline: none;
  }
</style>
