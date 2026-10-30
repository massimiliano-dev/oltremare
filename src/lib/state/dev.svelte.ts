// Vista «Sviluppo»: progetto aperto, file, differenze, conversazione di Claude dal vivo, processi.
import { api, errorText, type DevEntry, type DevFile, type DevGit, type DevProcess, type DevSession } from "../api";
import { on } from "../events";
import { ui } from "./ui.svelte";

const KEY = "oltremare:dev-folder";

export class DevState {
  /** Le istanze dello Studio (una per scheda) non toccano la cartella ricordata dalla finestra principale */
  constructor(private remember = true) {}

  folder = $state<string | null>(null);
  recent = $state<string[]>([]);
  tree = $state<Record<string, DevEntry[]>>({});
  expanded = $state<Record<string, boolean>>({});
  file = $state<DevFile | null>(null);
  tab = $state<"file" | "diff" | "preview" | "team">("file");
  diff = $state("");
  diffFile = $state<string | null>(null);
  git = $state<DevGit | null>(null);
  session = $state<DevSession | null>(null);
  sessionId = $state<string | null>(null);
  processes = $state<DevProcess[]>([]);
  previewUrl = $state<string | null>(null);
  sending = $state(false);
  bottom = $state<string | null>(null);
  private timers: ReturnType<typeof setInterval>[] = [];
  private suggested = new Set<string>();
  private unwatch: (() => void) | null = null;

  changed = $derived(new Map((this.git?.changes ?? []).map((c) => [c.path, c.status])));
  urls = $derived([...new Set(this.processes.flatMap((p) => p.urls))]);

  async init() {
    try {
      this.folder = localStorage.getItem(KEY);
    } catch {}
    this.recent = await api.devProjects().catch(() => []);
    if (!this.folder && this.recent[0]) this.folder = this.recent[0];
    on("dev-project", ({ folder }) => {
      this.recent = [folder, ...this.recent.filter((x) => x !== folder)].slice(0, 12);
      if (ui.view === "dev") {
        if (folder !== this.folder) this.open(folder);
      } else if (!this.suggested.has(folder)) {
        this.suggested.add(folder);
        this.folder = folder;
        ui.toast(`Lavoro in corso su ${folder.split(/[\\/]/).pop()}`, "info", { label: "Segui", run: () => (ui.view = "dev") }, 8000);
      }
    });
    on("dev-session", ({ folder }) => {
      if (folder === this.folder) this.refreshSession();
    });
  }

  /** Avvia gli aggiornamenti mentre la vista è aperta. */
  start() {
    this.stop();
    if (this.folder) this.loadAll();
    this.timers.push(setInterval(() => this.refreshSession(), 1500));
    this.timers.push(setInterval(() => this.refreshProcesses(), 2000));
    this.timers.push(setInterval(() => this.refreshGit(), 4000));
  }

  stop() {
    for (const t of this.timers) clearInterval(t);
    this.timers = [];
    this.unwatch?.();
    this.unwatch = null;
  }

  /** Segue la sessione di Claude di questa cartella (per le istanze che non passano da init). */
  async watch() {
    this.unwatch?.();
    this.unwatch = await on("dev-session", ({ folder }) => {
      if (folder === this.folder) this.refreshSession();
    });
  }

  async open(folder: string) {
    try {
      this.folder = await api.devOpen(folder);
      if (this.remember) localStorage.setItem(KEY, this.folder);
    } catch (e) {
      ui.toast(errorText(e), "error");
      return;
    }
    this.tree = {};
    this.expanded = {};
    this.file = null;
    this.diff = "";
    this.diffFile = null;
    this.sessionId = null;
    this.loadAll();
  }

  async pick() {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const r = await open({ directory: true, title: "Progetto da aprire" });
      if (typeof r === "string") await this.open(r);
    } catch {}
  }

  private loadAll() {
    if (!this.folder) return;
    this.loadDir(this.folder);
    this.refreshGit();
    this.refreshSession();
    this.refreshProcesses();
  }

  async loadDir(path: string) {
    this.tree[path] = await api.devTree(path).catch(() => []);
  }

  async toggleDir(path: string) {
    this.expanded[path] = !this.expanded[path];
    if (this.expanded[path] && !this.tree[path]) await this.loadDir(path);
  }

  async openFile(path: string) {
    try {
      this.file = await api.devFile(path);
      this.tab = this.changed.has(path) ? "diff" : "file";
      if (this.tab === "diff") await this.showDiff(path);
    } catch (e) {
      ui.toast(errorText(e), "error");
    }
  }

  async showDiff(file?: string) {
    if (!this.folder) return;
    this.diffFile = file ?? null;
    this.tab = "diff";
    this.diff = await api.devDiff(this.folder, file).catch((e) => errorText(e));
  }

  async refreshGit() {
    if (this.folder) this.git = await api.devGit(this.folder).catch(() => null);
  }

  async refreshSession() {
    if (!this.folder) return;
    const s = await api.devSession(this.folder, this.sessionId ?? undefined).catch(() => null);
    if (s) this.session = s;
  }

  async refreshProcesses() {
    const all = await api.devProcesses().catch(() => []);
    this.processes = this.folder ? all.filter((p) => p.folder.startsWith(this.folder!)) : all;
    if (!this.previewUrl && this.urls[0]) this.previewUrl = this.urls[0];
  }

  async send(text: string, opts: { newSession?: boolean; here: boolean }) {
    if (!this.folder || !text.trim() || this.sending) return false;
    this.sending = true;
    try {
      const r = await api.devSend(this.folder, text.trim(), { session: opts.newSession ? undefined : (this.session?.id ?? undefined), newSession: opts.newSession, here: opts.here });
      if (opts.newSession) this.sessionId = null;
      ui.toast(r.split(".")[0], "info");
      setTimeout(() => this.refreshSession(), 1500);
      return true;
    } catch (e) {
      ui.toast(errorText(e), "error");
      return false;
    } finally {
      this.sending = false;
    }
  }

  async stopClaude() {
    if (this.folder) await api.devStop(this.folder).catch(() => {});
    this.refreshSession();
  }
}

export const dev = new DevState();
