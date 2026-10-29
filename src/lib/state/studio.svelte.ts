// Oltremare Studio: progetti, file aperti nell'editor, orchestratore (modello, piano, AI scelta).
import { api, errorText, type ClaudexInfo, type StudioDecision, type StudioProject } from "../api";
import { dev } from "./dev.svelte";
import { git } from "./git.svelte";
import { ui } from "./ui.svelte";

const PIN = "oltremare:studio-pin";
const RECENT = "oltremare:studio-recent";

export interface OpenFile {
  path: string;
  name: string;
  content: string;
  saved: string;
  binary: boolean;
}

export interface Task {
  id: number;
  text: string;
  decision: StudioDecision | null;
  model: string;
  plan: boolean;
  profile: string;
  session: string | null;
  phase: "deciding" | "planning" | "plan-ready" | "working" | "done" | "error";
  planText?: string;
  opinion?: string;
  error?: string;
}

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
  project = $state<StudioProject | null>(null);
  pinned = $state<string[]>(load(PIN, []));
  recent = $state<string[]>(load(RECENT, []));
  files = $state<OpenFile[]>([]);
  active = $state<string | null>(null);
  claudex = $state<ClaudexInfo | null>(null);
  tasks = $state<Task[]>([]);
  /** Scelte per la prossima richiesta: «auto» lascia decidere all'orchestratore */
  modelChoice = $state<"auto" | "haiku" | "sonnet" | "opus" | "fable">("auto");
  planChoice = $state<"auto" | "yes" | "no">("auto");
  profile = $state("");
  draft = $state("");
  /** Nota del progetto nella memoria (Obsidian), se c'è */
  memoryNote = $state<string | null>(null);
  /** «Nuova conversazione»: la prossima richiesta non continua la sessione precedente */
  fresh = $state(false);
  private taskId = 0;

  current = $derived(this.files.find((f) => f.path === this.active) ?? null);
  lastTask = $derived(this.tasks.at(-1) ?? null);
  /** Messaggi scritti mentre Claude lavora: partono in ordine, nella stessa sessione, appena finisce */
  queue = $state<{ id: number; text: string }[]>([]);
  private queueId = 0;
  /** Claude lavora, o c'è un piano da approvare: i nuovi messaggi aspettano */
  busy = $derived(!!dev.session?.running || ["deciding", "planning", "working", "plan-ready"].includes(this.tasks.at(-1)?.phase ?? ""));

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

  async open(p: StudioProject | string) {
    const proj = typeof p === "string" ? (this.projects.find((x) => x.path === p) ?? this.fake(p)) : p;
    this.project = proj;
    this.recent = [proj.path, ...this.recent.filter((x) => x !== proj.path)].slice(0, 8);
    localStorage.setItem(RECENT, JSON.stringify(this.recent));
    this.files = [];
    this.active = null;
    this.tasks = [];
    await dev.open(proj.path);
    // La conversazione del progetto riprende da dove era rimasta
    this.restore(proj.path);
    dev.start();
    git.start(proj.path);
    document.title = `${proj.name} · Oltremare Studio`;
  }

  close() {
    this.queue = [];
    this.memoryNote = null;
    dev.stop();
    git.stop();
    this.project = null;
    this.files = [];
    this.tasks = [];
    document.title = "Oltremare Studio";
    this.loadProjects();
  }

  private fake(path: string): StudioProject {
    const name = path.split(/[\\/]/).filter(Boolean).pop() ?? path;
    return { name, group: "", root: "", path, stack: [], branch: "", changes: -1, updatedAt: Date.now(), sessions: 0, lastSession: null, icon: null, hue: 275, readme: null };
  }

  // ---------- Editor ----------

  async openFile(path: string) {
    if (!this.files.some((f) => f.path === path)) {
      try {
        const f = await api.devFile(path);
        this.files = [...this.files, { path, name: path.split(/[\\/]/).pop() ?? path, content: f.content, saved: f.content, binary: f.binary }];
      } catch (e) {
        ui.toast(errorText(e), "error");
        return;
      }
    }
    this.active = path;
    dev.tab = "file";
  }

  closeFile(path: string) {
    const i = this.files.findIndex((f) => f.path === path);
    this.files = this.files.filter((f) => f.path !== path);
    if (this.active === path) this.active = this.files[Math.max(0, i - 1)]?.path ?? null;
  }

  async save(path = this.active) {
    const f = this.files.find((x) => x.path === path);
    if (!f || f.content === f.saved) return;
    try {
      await api.studioSave(f.path, f.content);
      f.saved = f.content;
      dev.refreshGit();
    } catch (e) {
      ui.toast(errorText(e), "error");
    }
  }

  /** Dopo le modifiche di Claude: i file aperti e non toccati si ricaricano. */
  async reloadClean() {
    for (const f of this.files) {
      if (f.content !== f.saved || f.binary) continue;
      const n = await api.devFile(f.path).catch(() => null);
      if (n && n.content !== f.saved) {
        f.content = n.content;
        f.saved = n.content;
      }
    }
  }

  // ---------- Orchestratore ----------

  async send(text: string, newSession = false) {
    const proj = this.project;
    if (!proj || !text.trim()) return;
    const task: Task = { id: ++this.taskId, text: text.trim(), decision: null, model: this.modelChoice, plan: this.planChoice === "yes", profile: this.profile, session: null, phase: "deciding" };
    this.tasks = [...this.tasks, task];
    const t = this.tasks[this.tasks.length - 1];
    try {
      // L'orchestratore decide modello e piano se l'utente non ha scelto
      if (this.modelChoice === "auto" || this.planChoice === "auto") {
        t.decision = await api.studioClassify(proj.path, t.text);
        if (this.modelChoice === "auto") t.model = t.decision.model;
        if (this.planChoice === "auto") t.plan = t.decision.plan;
      }
      t.phase = t.plan ? "planning" : "working";
      const nuova = newSession || this.fresh;
      this.fresh = false;
      const r = await api.studioRun(proj.path, t.text, t.model, t.plan, t.profile || undefined, nuova ? undefined : (dev.sessionId ?? dev.session?.id ?? undefined), nuova);
      t.session = r.session;
      dev.sessionId = r.session || null;
      dev.refreshSession();
    } catch (e) {
      t.phase = "error";
      t.error = errorText(e);
    }
  }

  /** Compiti dell'orchestratore salvati per progetto (gli ultimi 30). */
  persist() {
    if (!this.project) return;
    try {
      localStorage.setItem(`oltremare:studio-tasks:${this.project.path}`, JSON.stringify({ tasks: this.tasks.slice(-30), fresh: this.fresh }));
    } catch {}
  }

  private restore(path: string) {
    const saved = load<{ tasks: Task[]; fresh?: boolean } | null>(`oltremare:studio-tasks:${path}`, null);
    // Ciò che era a metà quando lo Studio si è chiuso non può riprendere: si mostra come concluso
    this.tasks = (saved?.tasks ?? []).map((t) => (["deciding", "planning", "working"].includes(t.phase) ? { ...t, phase: "done" as const } : t));
    this.fresh = !!saved?.fresh;
    this.taskId = Math.max(0, ...this.tasks.map((t) => t.id));
    const sid = [...this.tasks].reverse().find((t) => t.session)?.session;
    if (sid && !this.fresh) dev.sessionId = sid;
    this.refreshMemory();
  }

  async refreshMemory() {
    if (!this.project) return;
    this.memoryNote = (await api.memoryInfo(this.project.path).catch(() => null))?.projectNote ?? null;
  }

  /** Nuova conversazione: si svuota la vista e la prossima richiesta parte da una sessione nuova. */
  newConversation() {
    dev.sessionId = null;
    this.tasks = [];
    this.queue = [];
    this.fresh = true;
  }

  enqueue(text: string) {
    if (text.trim()) this.queue = [...this.queue, { id: ++this.queueId, text: text.trim() }];
  }

  unqueue(id: number) {
    this.queue = this.queue.filter((q) => q.id !== id);
  }

  /** Manda il primo messaggio in coda, se Claude è libero. */
  drain() {
    if (this.busy || !this.queue.length) return;
    const [q, ...rest] = this.queue;
    this.queue = rest;
    this.send(q.text);
  }

  /** Interrompe il lavoro in corso (o lascia il piano) e manda subito questo messaggio. */
  async sendNow(id: number) {
    const q = this.queue.find((x) => x.id === id);
    if (!q) return;
    this.queue = [q, ...this.queue.filter((x) => x.id !== id)];
    const t = this.lastTask;
    if (dev.session?.running) {
      await dev.stopClaude();
      return; // onFinished chiude il compito e svuota la coda
    }
    if (t && t.phase !== "done" && t.phase !== "error") t.phase = "done";
    this.drain();
  }

  /** Quando Claude finisce: piano pronto da approvare, oppure lavoro concluso. */
  async onFinished() {
    const t = this.lastTask;
    if (!t || !this.project) return;
    if (t.phase === "planning" && t.session) {
      const r = await api.studioPlan(this.project.path, t.session).catch(() => ({ plan: "" }));
      t.planText = r.plan;
      t.phase = "plan-ready";
    } else if (t.phase === "working") {
      t.phase = "done";
      this.reloadClean();
      // La nota del progetto in memoria nasce (o si aggiorna) a fine lavoro
      this.refreshMemory();
    }
    this.drain();
  }

  async approve(t: Task, notes?: string) {
    if (!this.project || !t.session) return;
    try {
      t.phase = "working";
      await api.studioApprove(this.project.path, t.session, t.model === "auto" ? "sonnet" : t.model, t.profile || undefined, notes);
      dev.refreshSession();
    } catch (e) {
      t.phase = "error";
      t.error = errorText(e);
    }
  }

  async replan(t: Task, notes: string) {
    if (!this.project) return;
    t.phase = "planning";
    try {
      const r = await api.studioRun(this.project.path, `Rivedi il piano tenendo conto di queste indicazioni: ${notes}`, t.model, true, t.profile || undefined, t.session ?? undefined);
      t.session = r.session;
    } catch (e) {
      t.phase = "error";
      t.error = errorText(e);
    }
  }

  async opinion(t: Task, profile: string, about: "plan" | "diff") {
    if (!this.project) return;
    t.opinion = "…";
    try {
      t.opinion = await api.studioSecondOpinion(this.project.path, profile, about, t.planText);
    } catch (e) {
      t.opinion = `Non è riuscito: ${errorText(e)}`;
    }
  }
}

export const studio = new StudioState();
