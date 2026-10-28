<script lang="ts">
  // Editor di codice (CodeMirror 6): evidenziazione per linguaggio, numeri di riga, ricerca, ⌘S per salvare.
  import { onMount } from "svelte";
  import type { EditorView } from "codemirror";
  import { theme } from "../state/theme.svelte";

  let { path, value, onchange, onsave }: { path: string; value: string; onchange: (v: string) => void; onsave: () => void } = $props();
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
      const [{ EditorView, basicSetup }, { keymap }, { EditorState }, { oneDark }] = await Promise.all([
        import("codemirror"),
        import("@codemirror/view"),
        import("@codemirror/state"),
        import("@codemirror/theme-one-dark"),
      ]);
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
            keymap.of([{ key: "Mod-s", preventDefault: true, run: () => (onsave(), true) }]),
            chiaro,
            ...(theme.resolved === "dark" ? [oneDark] : []),
            ...(lang ? [lang] : []),
            EditorView.updateListener.of((u) => {
              if (u.docChanged && !applying) onchange(u.state.doc.toString());
            }),
          ],
        }),
      });
    })();
    return () => {
      disposed = true;
      view?.destroy();
    };
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
