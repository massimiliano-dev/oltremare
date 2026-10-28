// Tema chiaro/scuro: attributo `data-theme` sulla radice, applicato prima del primo disegno.
import { getCurrentWindow } from "@tauri-apps/api/window";

export type ThemeMode = "system" | "light" | "dark";
const KEY = "oltremare:theme";
const LOOK = "oltremare:look";

export interface Look {
  accentHue: number;
  accentChroma: number;
  density: "comfortable" | "compact";
  fontScale: number;
}

/** Preset di colore: tonalità e saturazione dell'accento (il fondo resta tinto appena). */
export const ACCENTS: { id: string; label: string; hue: number; chroma: number }[] = [
  { id: "oltremare", label: "Oltremare", hue: 275, chroma: 0.2 },
  { id: "cobalto", label: "Cobalto", hue: 258, chroma: 0.19 },
  { id: "petrolio", label: "Petrolio", hue: 215, chroma: 0.12 },
  { id: "salvia", label: "Salvia", hue: 155, chroma: 0.1 },
  { id: "ambra", label: "Ambra", hue: 70, chroma: 0.14 },
  { id: "corallo", label: "Corallo", hue: 28, chroma: 0.17 },
  { id: "prugna", label: "Prugna", hue: 330, chroma: 0.15 },
  { id: "grafite", label: "Grafite", hue: 270, chroma: 0.03 },
];

class ThemeState {
  mode = $state<ThemeMode>("system");
  systemDark = $state(typeof matchMedia === "function" && matchMedia("(prefers-color-scheme: dark)").matches);
  resolved = $derived<"light" | "dark">(this.mode === "system" ? (this.systemDark ? "dark" : "light") : this.mode);

  boot() {
    try {
      const saved = localStorage.getItem(KEY) as ThemeMode | null;
      if (saved === "light" || saved === "dark" || saved === "system") this.mode = saved;
      const look = JSON.parse(localStorage.getItem(LOOK) ?? "null") as Look | null;
      if (look) this.look(look, false);
    } catch {}
    this.apply();
    matchMedia("(prefers-color-scheme: dark)").addEventListener("change", (e) => (this.systemDark = e.matches));
    $effect.root(() => {
      $effect(() => this.apply());
    });
  }

  set(mode: ThemeMode) {
    if (mode === this.mode) return;
    const html = document.documentElement;
    if (!matchMedia("(prefers-reduced-motion: reduce)").matches) {
      html.classList.add("switching");
      setTimeout(() => html.classList.remove("switching"), 300);
    }
    this.mode = mode;
    try {
      localStorage.setItem(KEY, mode);
    } catch {}
  }

  /** Accento, densità e dimensione del testo: variabili CSS sulla radice, ricordate per il prossimo avvio. */
  look(l: Partial<Look>, remember = true) {
    const st = document.documentElement.style;
    if (l.accentHue != null) st.setProperty("--accent-h", String(l.accentHue));
    if (l.accentChroma != null) st.setProperty("--accent-c", String(l.accentChroma));
    if (l.accentHue != null) st.setProperty("--tint-h", String(l.accentHue - 3));
    if (l.fontScale != null) st.setProperty("--font-scale", String(l.fontScale));
    if (l.density) document.documentElement.dataset.density = l.density;
    if (remember) {
      try {
        const prev = JSON.parse(localStorage.getItem(LOOK) ?? "{}");
        localStorage.setItem(LOOK, JSON.stringify({ ...prev, ...l }));
      } catch {}
    }
  }

  apply() {
    const r = this.resolved;
    document.documentElement.dataset.theme = r;
    // Barra del titolo e sfondo della finestra coerenti con il tema (niente bordi bianchi ridimensionando)
    const w = (() => {
      try {
        return getCurrentWindow();
      } catch {
        return null;
      }
    })();
    w?.setTheme(this.mode === "system" ? null : r).catch(() => {});
    w?.setBackgroundColor(r === "dark" ? [21, 19, 34, 255] : [249, 248, 255, 255]).catch(() => {});
  }
}

export const theme = new ThemeState();
