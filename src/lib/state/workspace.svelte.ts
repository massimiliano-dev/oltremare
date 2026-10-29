// Uno spazio di lavoro dello Studio: un progetto aperto in una scheda, con il suo albero dei file,
// il suo git, l'editor, l'orchestratore (compiti, coda, revisione del CTO) e la memoria del progetto.
import { api, errorText, type StudioDecision, type StudioProject, type StudioReview } from "../api";
import { DevState } from "./dev.svelte";
import { GitStore } from "./git.svelte";
import { ui } from "./ui.svelte";
import { studio } from "./studio.svelte";

export interface OpenFile {
  path: string;
  name: string;
  content: string;
  saved: string;
  binary: boolean;
}

export type Phase = "deciding" | "planning" | "plan-ready" | "working" | "reviewing" | "review-ready" | "done" | "error";

export interface Task {
  id: number;
  text: string;
  decision: StudioDecision | null;
  model: string;
  plan: boolean;
  profile: string;
  session: string | null;
  phase: Phase;
  planText?: string;
  opinion?: string;
  error?: string;
  /** Revisione: sessione in corso, verdetti dei revisori (CTO e, se c'è, un'altra AI), verdetto unito,
   * quante volte le correzioni sono già partite da sole */
  reviewSession?: string | null;
  reviews?: { who: string; profile: string; review: StudioReview }[];
  review?: StudioReview | null;
  fixRounds?: number;
}

/** Il secondo revisore tra i profili di Claudex: prima GPT/Codex, poi le altre AI remote, poi i modelli locali più grandi. */
export function secondReviewer(exclude: string): { name: string; model: string } | null {
  const profiles = studio.claudex?.profiles ?? [];
  const score = (p: { name: string; provider: string; model: string }) => {
    const n = `${p.name} ${p.model}`.toLowerCase();
    if (p.provider === "OpenAIResponses" || /gpt|codex|openai|\bo[134]\b/.test(n)) return 100;
    if (/gemini|grok|deepseek|kimi|glm|mistral|claude|minimax|openrouter/.test(n)) return 80;
    const size = Number(n.match(/(\d+)b\b/)?.[1] ?? 0);
    return 20 + Math.min(size, 70);
  };
  return profiles.filter((p) => p.name !== exclude).sort((a, b) => score(b) - score(a))[0] ?? null;
}

/** Un verdetto solo dai revisori: basta un «da correggere» con problemi seri; i problemi si sommano. */
function combine(reviews: { who: string; review: StudioReview }[]): StudioReview {
  const problemi: StudioReview["problemi"] = [];
  for (const r of reviews) {
    for (const p of r.review.problemi) {
      const key = `${p.file}:${p.riga}:${p.problema.slice(0, 40).toLowerCase()}`;
      if (!problemi.some((x) => `${x.file}:${x.riga}:${x.problema.slice(0, 40).toLowerCase()}` === key)) problemi.push(p);
    }
  }
  const gravi = problemi.some((p) => p.gravita !== "minore");
  const voti = reviews.map((r) => r.review.voto).filter((v): v is number => !!v);
  return {
    esito: gravi ? "da correggere" : reviews.every((r) => r.review.esito === "sconosciuto") ? "sconosciuto" : "approvato",
    voto: voti.length ? Math.min(...voti) : null,
    sintesi: reviews.map((r) => r.review.sintesi).filter(Boolean).join(" · "),
    testo: reviews.map((r) => r.review.testo ?? "").filter(Boolean).join("\n\n"),
    problemi,
  };
}

function load<T>(k: string, d: T): T {
  try {
    return JSON.parse(localStorage.getItem(k) ?? "null") ?? d;
  } catch {
    return d;
  }
}

let seq = 0;

export class Workspace {
  readonly id = `ws-${++seq}`;
  readonly dev = new DevState(false);
  readonly git: GitStore;
  project = $state<StudioProject>(undefined as unknown as StudioProject);
  files = $state<OpenFile[]>([]);
  active = $state<string | null>(null);
  tasks = $state<Task[]>([]);
  /** Scelte per la prossima richiesta: «auto» lascia decidere all'orchestratore */
  modelChoice = $state<"auto" | "haiku" | "sonnet" | "opus" | "fable">("auto");
  planChoice = $state<"auto" | "yes" | "no">("auto");
  /** Revisione del CTO dopo il lavoro: «auto» rivede tutto tranne le domande */
  reviewChoice = $state<"auto" | "yes" | "no">("auto");
  profile = $state("");
  draft = $state("");
  /** Nota del progetto nella memoria (Obsidian), se c'è */
  memoryNote = $state<string | null>(null);
  /** «Nuova conversazione»: la prossima richiesta non continua la sessione precedente */
  fresh = $state(false);
  /** Messaggi scritti mentre Claude lavora: partono in ordine, nella stessa sessione, appena finisce */
  queue = $state<{ id: number; text: string }[]>([]);
  private taskId = 0;
  private queueId = 0;

  current = $derived(this.files.find((f) => f.path === this.active) ?? null);
  lastTask = $derived(this.tasks.at(-1) ?? null);
  /** Claude (o il CTO) lavora, o c'è un piano da approvare: i nuovi messaggi aspettano */
  busy = $derived(!!this.dev.session?.running || ["deciding", "planning", "working", "plan-ready", "reviewing"].includes(this.tasks.at(-1)?.phase ?? ""));

  constructor(project: StudioProject) {
    this.project = project;
    this.git = new GitStore(this.dev);
  }

  get path() {
    return this.project.path;
  }

  async start() {
    await this.dev.open(this.project.path);
    this.restore();
    this.dev.start();
    this.dev.watch();
    this.git.start(this.project.path);
  }

  stop() {
    this.dev.stop();
    this.git.stop();
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
    this.dev.tab = "file";
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
      this.dev.refreshGit();
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
    if (!text.trim()) return;
    const task: Task = { id: ++this.taskId, text: text.trim(), decision: null, model: this.modelChoice, plan: this.planChoice === "yes", profile: this.profile, session: null, phase: "deciding", fixRounds: 0 };
    this.tasks = [...this.tasks, task];
    const t = this.tasks[this.tasks.length - 1];
    try {
      // L'orchestratore decide modello e piano se l'utente non ha scelto
      if (this.modelChoice === "auto" || this.planChoice === "auto") {
        t.decision = await api.studioClassify(this.path, t.text);
        if (this.modelChoice === "auto") t.model = t.decision.model;
        if (this.planChoice === "auto") t.plan = t.decision.plan;
      }
      t.phase = t.plan ? "planning" : "working";
      const nuova = newSession || this.fresh;
      this.fresh = false;
      const r = await api.studioRun(this.path, t.text, t.model, t.plan, t.profile || undefined, nuova ? undefined : (this.dev.sessionId ?? this.dev.session?.id ?? undefined), nuova);
      t.session = r.session;
      this.dev.sessionId = r.session || null;
      this.dev.refreshSession();
    } catch (e) {
      t.phase = "error";
      t.error = errorText(e);
      this.drain();
    }
  }

  /** Compiti dell'orchestratore salvati per progetto (gli ultimi 30). */
  persist() {
    try {
      localStorage.setItem(`oltremare:studio-tasks:${this.path}`, JSON.stringify({ tasks: this.tasks.slice(-30), fresh: this.fresh }));
    } catch {}
  }

  private restore() {
    const saved = load<{ tasks: Task[]; fresh?: boolean } | null>(`oltremare:studio-tasks:${this.path}`, null);
    // Ciò che era a metà quando lo Studio si è chiuso non può riprendere: si mostra come concluso
    this.tasks = (saved?.tasks ?? []).map((t) => (["deciding", "planning", "working", "reviewing"].includes(t.phase) ? { ...t, phase: "done" as const } : t));
    this.fresh = !!saved?.fresh;
    this.taskId = Math.max(0, ...this.tasks.map((t) => t.id));
    const sid = [...this.tasks].reverse().find((t) => t.session)?.session;
    if (sid && !this.fresh) this.dev.sessionId = sid;
    this.refreshMemory();
  }

  async refreshMemory() {
    this.memoryNote = (await api.memoryInfo(this.path).catch(() => null))?.projectNote ?? null;
  }

  /** Nuova conversazione: si svuota la vista e la prossima richiesta parte da una sessione nuova. */
  newConversation() {
    this.dev.sessionId = null;
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
    if (this.dev.session?.running) {
      await this.dev.stopClaude();
      return; // onFinished chiude il compito e svuota la coda
    }
    if (t && t.phase !== "done" && t.phase !== "error") t.phase = "done";
    this.drain();
  }

  /** Quando Claude finisce: piano pronto da approvare, lavoro da far rivedere al CTO, oppure verdetto arrivato. */
  async onFinished() {
    const t = this.lastTask;
    if (!t) return;
    if (t.phase === "planning" && t.session) {
      const r = await api.studioPlan(this.path, t.session).catch(() => ({ plan: "" }));
      t.planText = r.plan;
      t.phase = "plan-ready";
    } else if (t.phase === "working") {
      this.reloadClean();
      // La nota del progetto in memoria nasce (o si aggiorna) a fine lavoro
      this.refreshMemory();
      if (await this.wantsReview(t)) await this.startReview(t);
      else t.phase = "done";
    } else if (t.phase === "reviewing" && t.reviewSession) {
      const r = (await api.studioReviewResult(this.path, t.reviewSession).catch(() => null)) ?? { esito: "sconosciuto" as const, sintesi: "", problemi: [], voto: null, testo: "Non sono riuscito a leggere il verdetto." };
      const reviews = [...(t.reviews ?? [])];
      const second = secondReviewer(t.profile);
      const who = reviews.length ? `Secondo revisore · ${second?.name ?? "altra AI"}` : `CTO · ${this.reviewer(t) === "fable" ? "Fable" : "Opus"}`;
      reviews.push({ who, profile: reviews.length ? (second?.name ?? "") : "", review: r });
      t.reviews = reviews;
      t.reviewSession = null;
      // Dopo il CTO, se c'è un'altra AI disponibile, rivede anche lei (una sola volta per tornata)
      if (reviews.length === 1 && second && this.reviewChoice !== "no") {
        try {
          const r2 = await api.studioReview(this.path, t.text, "auto", second.name);
          t.reviewSession = r2.session;
          return; // si resta in «reviewing»: al prossimo onFinished arriva il secondo verdetto
        } catch (e) {
          ui.toast(`Secondo revisore (${second.name}) non partito: ${errorText(e)}`, "error");
        }
      }
      t.review = combine(reviews);
      if (t.review.esito === "da correggere" && (t.fixRounds ?? 0) < 1) {
        // Prima tornata di correzioni da sola; poi decide l'utente
        t.fixRounds = (t.fixRounds ?? 0) + 1;
        await this.fix(t);
      } else if (t.review.esito === "da correggere") {
        t.phase = "review-ready";
      } else {
        t.phase = "done";
      }
    }
    this.drain();
  }

  /** Il CTO rivede tutto ciò che ha toccato file (non le domande), a meno che l'utente scelga altrimenti. */
  private async wantsReview(t: Task) {
    if (this.reviewChoice === "no") return false;
    if (t.profile && this.reviewChoice !== "yes") return false; // con un'altra AI si rivede solo su richiesta
    await this.git.refresh();
    const cambiati = (this.git.state?.staged?.length ?? 0) + (this.git.state?.changed?.length ?? 0);
    if (!this.git.state?.repo || !cambiati) return false;
    if (this.reviewChoice === "yes") return true;
    return t.decision?.kind !== "domanda";
  }

  private reviewer(t: Task) {
    return t.model === "fable" ? "fable" : "opus";
  }

  async startReview(t: Task) {
    t.phase = "reviewing";
    t.reviews = [];
    t.review = null;
    try {
      const r = await api.studioReview(this.path, t.text, this.reviewer(t), t.profile || undefined);
      t.reviewSession = r.session;
    } catch (e) {
      t.phase = "done";
      ui.toast(`Revisione del CTO non partita: ${errorText(e)}`, "error");
    }
  }

  /** Rimanda allo sviluppatore i punti del CTO, nella stessa sessione del lavoro. */
  async fix(t: Task) {
    if (!t.review) return;
    const righe = t.review.problemi.filter((p) => p.gravita !== "minore").map((p) => `- [${p.gravita}] ${p.file}${p.riga ? `:${p.riga}` : ""} — ${p.problema}${p.correzione ? `\n  Correzione: ${p.correzione}` : ""}`);
    const minori = t.review.problemi.filter((p) => p.gravita === "minore").map((p) => `- ${p.file}${p.riga ? `:${p.riga}` : ""} — ${p.problema}`);
    const testo = `Il CTO ha rivisto le modifiche e chiede correzioni.\n${t.review.sintesi ? `Giudizio: ${t.review.sintesi}\n` : ""}\nDa correggere:\n${righe.join("\n")}${minori.length ? `\n\nMinori (sistemali se è rapido):\n${minori.join("\n")}` : ""}\n\nCorreggi questi punti, poi verifica di nuovo (build e test) e riepiloga cosa hai cambiato.`;
    t.phase = "working";
    try {
      const r = await api.studioRun(this.path, testo, t.model, false, t.profile || undefined, t.session ?? undefined, false);
      t.session = r.session;
      this.dev.sessionId = r.session || null;
    } catch (e) {
      t.phase = "error";
      t.error = errorText(e);
    }
  }

  /** L'utente accetta il lavoro così com'è, nonostante i rilievi. */
  accept(t: Task) {
    if (t.phase === "review-ready") t.phase = "done";
  }

  async approve(t: Task, notes?: string) {
    if (!t.session) return;
    try {
      t.phase = "working";
      await api.studioApprove(this.path, t.session, t.model === "auto" ? "sonnet" : t.model, t.profile || undefined, notes);
      this.dev.refreshSession();
    } catch (e) {
      t.phase = "error";
      t.error = errorText(e);
    }
  }

  async replan(t: Task, notes: string) {
    t.phase = "planning";
    try {
      const r = await api.studioRun(this.path, `Rivedi il piano tenendo conto di queste indicazioni: ${notes}`, t.model, true, t.profile || undefined, t.session ?? undefined);
      t.session = r.session;
    } catch (e) {
      t.phase = "error";
      t.error = errorText(e);
    }
  }

  async opinion(t: Task, profile: string, about: "plan" | "diff") {
    t.opinion = "…";
    try {
      t.opinion = await api.studioSecondOpinion(this.path, profile, about, t.planText);
    } catch (e) {
      t.opinion = `Non è riuscito: ${errorText(e)}`;
    }
  }
}
