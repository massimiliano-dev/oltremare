// Oltremare Studio: l'elenco dei progetti e le schede aperte (uno spazio di lavoro per progetto).
import { api, errorText, type ClaudexInfo, type StudioProject } from "../api";
import { ui } from "./ui.svelte";
import { Workspace } from "./workspace.svelte";

export type { OpenFile, Task, Phase } from "./workspace.svelte";
export { Workspace };

const PIN = "oltremare:studio-pin";
const RECENT = "oltremare:studio-recent";
const TABS = "oltremare:studio-tabs";

function load<T>(k: string, d: T): T {
  try {
    return JSON.parse(localStorage.getItem(k) ?? "null") ?? d;
  } catch {
    return d;
  }
}

class StudioState {
  projects = $state<StudioProject[]>([]);
  loading = $state(false);
  pinned = $state<string[]>(load(PIN, []));
  recent = $state<string[]>(load(RECENT, []));
  claudex = $state<ClaudexInfo | null>(null);
  /** Le schede aperte; `tab` è quella in primo piano (null = l'elenco dei progetti) */
  tabs = $state<Workspace[]>([]);
  tab = $state<Workspace | null>(null);

  async loadProjects() {
    this.loading = true;
    try {
      this.projects = await api.studioProjects();
    } catch (e) {
      ui.toast(errorText(e), "error");
    } finally {
      this.loading = false;
    }
  }

  async loadClaudex() {
    this.claudex = await api.studioClaudex().catch(() => null);
  }

  togglePin(path: string) {
    this.pinned = this.pinned.includes(path) ? this.pinned.filter((x) => x !== path) : [path, ...this.pinned];
    localStorage.setItem(PIN, JSON.stringify(this.pinned));
  }

  /** Apre il progetto in una scheda (o porta davanti quella che c'è già). */
  async open(p: StudioProject | string, opts: { prompt?: string } = {}) {
    const path = typeof p === "string" ? p : p.path;
    let ws = this.tabs.find((t) => t.path === path);
    if (!ws) {
      const proj = typeof p === "string" ? (this.projects.find((x) => x.path === p) ?? this.fake(p)) : p;
      ws = new Workspace(proj);
      this.tabs = [...this.tabs, ws];
      this.recent = [path, ...this.recent.filter((x) => x !== path)].slice(0, 8);
      localStorage.setItem(RECENT, JSON.stringify(this.recent));
      this.saveTabs();
      await ws.start();
    }
    this.show(ws);
    if (opts.prompt) ws.draft = opts.prompt;
  }

  show(ws: Workspace | null) {
    this.tab = ws;
    document.title = ws ? `${ws.project.name} · Oltremare Studio` : "Oltremare Studio";
    this.saveTabs();
  }

  /** Chiude la scheda: si fermano aggiornamenti e terminali; Claude, se lavora, continua in background. */
  closeTab(ws: Workspace) {
    const i = this.tabs.indexOf(ws);
    ws.stop();
    this.tabs = this.tabs.filter((t) => t !== ws);
    if (this.tab === ws) this.show(this.tabs[Math.min(i, this.tabs.length - 1)] ?? null);
    else this.saveTabs();
    if (!this.tabs.length) this.loadProjects();
  }

  /** L'elenco dei progetti, con le schede che restano aperte. */
  picker() {
    this.show(null);
    this.loadProjects();
  }

  /** Scheda accanto (⌘⇧] e ⌘⇧[) */
  cycle(dir: 1 | -1) {
    if (!this.tabs.length) return;
    const i = this.tab ? this.tabs.indexOf(this.tab) : -1;
    this.show(this.tabs[(i + dir + this.tabs.length) % this.tabs.length]);
  }

  /** Le schede dell'ultima volta tornano all'apertura dello Studio. */
  restoreTabs() {
    const saved = load<{ paths: string[]; active: string | null }>(TABS, { paths: [], active: null });
    for (const path of saved.paths) {
      if (this.tabs.some((t) => t.path === path)) continue;
      const proj = this.projects.find((x) => x.path === path);
      if (!proj) continue;
      const ws = new Workspace(proj);
      this.tabs = [...this.tabs, ws];
      ws.start().catch(() => {});
    }
    const attiva = this.tabs.find((t) => t.path === saved.active) ?? null;
    if (attiva) this.show(attiva);
  }

  private saveTabs() {
    try {
      localStorage.setItem(TABS, JSON.stringify({ paths: this.tabs.map((t) => t.path), active: this.tab?.path ?? null }));
    } catch {}
  }

  private fake(path: string): StudioProject {
    const name = path.split(/[\\/]/).filter(Boolean).pop() ?? path;
    return { name, group: "", root: "", path, stack: [], branch: "", changes: -1, updatedAt: Date.now(), sessions: 0, lastSession: null, icon: null, hue: 275, readme: null };
  }
}

export const studio = new StudioState();
