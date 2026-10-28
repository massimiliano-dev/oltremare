// Disposizione dei pannelli: preset più larghezze regolabili, ricordate per ogni preset.
import { settings } from "./settings.svelte";

export type PanelKind = "agents" | "chat" | "side";

export interface LayoutPreset {
  id: string;
  label: string;
  sub: string;
  /** Colonne nell'ordine: larghezza in px (0 = elastica) */
  cols: { kind: PanelKind; width: number; flex: number }[];
}

export const LAYOUTS: LayoutPreset[] = [
  { id: "operator", label: "Operatore", sub: "agenti, chat, avatar e passaggi", cols: [{ kind: "agents", width: 248, flex: 1 }, { kind: "chat", width: 0, flex: 2.6 }, { kind: "side", width: 340, flex: 1.4 }] },
  { id: "chat", label: "Conversazione", sub: "agenti e chat", cols: [{ kind: "agents", width: 248, flex: 1 }, { kind: "chat", width: 0, flex: 4 }] },
  { id: "monitor", label: "Monitor", sub: "passaggi e notifiche in grande", cols: [{ kind: "agents", width: 220, flex: 1 }, { kind: "chat", width: 0, flex: 2 }, { kind: "side", width: 520, flex: 2 }] },
  { id: "focus", label: "Concentrazione", sub: "solo la chat", cols: [{ kind: "chat", width: 0, flex: 1 }] },
];

const KEY = "oltremare:layout-widths";
const LIMITS: Record<PanelKind, [number, number]> = { agents: [180, 380], chat: [360, 4000], side: [260, 760] };

class LayoutState {
  /** Larghezze personalizzate: preset → pannello → px */
  widths = $state<Record<string, Partial<Record<PanelKind, number>>>>({});
  /** Pannelli nascosti al volo (⌘B, ⌘J) */
  hidden = $state<Partial<Record<PanelKind, boolean>>>({});

  constructor() {
    try {
      this.widths = JSON.parse(localStorage.getItem(KEY) ?? "{}");
    } catch {}
  }

  preset = $derived(LAYOUTS.find((l) => l.id === settings.s?.layout) ?? LAYOUTS[0]);
  cols = $derived(this.preset.cols.filter((c) => !this.hidden[c.kind]));

  width(kind: PanelKind): number {
    const c = this.preset.cols.find((x) => x.kind === kind);
    return this.widths[this.preset.id]?.[kind] ?? c?.width ?? 0;
  }

  template = $derived(this.cols.map((c) => (c.kind === "chat" ? "minmax(0, 1fr)" : `${this.width(c.kind)}px`)).join(" "));

  resize(kind: PanelKind, px: number) {
    const [lo, hi] = LIMITS[kind];
    const id = this.preset.id;
    this.widths = { ...this.widths, [id]: { ...this.widths[id], [kind]: Math.round(Math.min(hi, Math.max(lo, px))) } };
    this.save();
  }

  reset(kind: PanelKind) {
    const id = this.preset.id;
    const next = { ...this.widths[id] };
    delete next[kind];
    this.widths = { ...this.widths, [id]: next };
    this.save();
  }

  toggle(kind: PanelKind) {
    // Se il preset non ha il pannello, si passa a quello completo
    if (!this.preset.cols.some((c) => c.kind === kind)) {
      settings.save({ layout: "operator" });
      this.hidden = { ...this.hidden, [kind]: false };
      return;
    }
    this.hidden = { ...this.hidden, [kind]: !this.hidden[kind] };
  }

  private save() {
    try {
      localStorage.setItem(KEY, JSON.stringify(this.widths));
    } catch {}
  }
}

export const layout = new LayoutState();
