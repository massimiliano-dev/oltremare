// Uno spazio di lavoro dello Studio: un progetto aperto in una scheda, con il suo albero dei file,
// il suo git, l'editor, l'orchestratore (compiti, coda, revisione del CTO) e la memoria del progetto.
import { on } from "../events";
import { api, errorText, type IdeProblem, type StudioPermission, type AgentDef, type StudioDecision, type StudioProject, type StudioReview, type TeamInfo } from "../api";
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
  /** Piano scartato dall'utente (resta visibile, ma non si esegue) */
  dropped?: boolean;
  /** File allegati alla richiesta */
  files?: string[];
  opinion?: string;
  error?: string;
  /** Revisione: sessione in corso, verdetti dei revisori (CTO e, se c'è, un'altra AI), verdetto unito,
   * quante volte le correzioni sono già partite da sole */
  reviewSession?: string | null;
  reviews?: { who: string; profile: string; review: StudioReview }[];
  review?: StudioReview | null;
  fixRounds?: number;
  /** Il lavoro si è fermato sulle proposte grafiche: la prossima risposta dell'utente è la scelta */
  proposals?: boolean;
}

/** Regola fissa dell'utente per il lavoro visivo: prima vedere come verrebbe, con più proposte. */
const PROPOSTE = `[Lavoro grafico: PRIMA LE PROPOSTE. Non modificare ancora il codice del progetto.
Prepara 3 proposte visive davvero diverse (direzione, composizione, tipografia, colore; non varianti di colore della stessa idea), anche se l'utente ha dato indicazioni precise: una le segue alla lettera, le altre le interpretano o le spingono oltre. Segui lo «Stile grafico» dell'utente e le skill impeccable (e design-taste-frontend se c'è); puoi lanciare in parallelo un sotto-agente oltremare-frontend per proposta.
Ogni proposta è una pagina HTML statica in .oltremare/proposte/<AAAAMMGG-HHMM>/proposta-N.html con contenuti, font e token veri del progetto (se la schermata esiste già, ridisegna quella con i suoi dati reali); controlla che .oltremare/.gitignore contenga «*».
Fai lo screenshot di ciascuna (1440×900, e 390×844 se il progetto è per telefono) con Chrome senza finestra: "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --headless=new --hide-scrollbars --window-size=1440,900 --screenshot=<file.png> file://<file.html>. Guarda le immagini e sistema ciò che non va prima di mostrarle.
Messaggio finale: per ogni proposta il nome, due righe sull'idea e il percorso assoluto dello screenshot (l'utente vede l'anteprima). Chiudi chiedendo quale preferisce o cosa mescolare, e fermati lì.]`;

const SCELTA = `[Risposta alle proposte grafiche appena mostrate in questa sessione. Se l'utente ne sceglie una (anche mescolandole o con correzioni), implementala nel progetto seguendo lo «Stile grafico» e le regole di sempre, aggiorna DESIGN.md se cambiano token, font o componenti, e verifica il risultato nel browser. Se invece chiede altre proposte o variazioni senza sceglierne una, preparane di nuove con le stesse regole e fermati di nuovo.]`;

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
  queue = $state<{ id: number; text: string; files: string[]; follow: boolean }[]>([]);
  /** Azioni delicate in attesa del tuo permesso (cancellazioni, force push…) */
  permissions = $state<StudioPermission[]>([]);
  private unlisten: (() => void)[] = [];
  /** La squadra al lavoro: sotto-agenti della sessione corrente, letti ogni pochi secondi */
  team = $state<TeamInfo | null>(null);
  /** Gli agenti definiti (di Oltremare e dell'utente) */
  agentDefs = $state<AgentDef[]>([]);
  private timers: ReturnType<typeof setInterval>[] = [];
  private taskId = 0;
  private queueId = 0;

  /** Mini IDE: riga a cui saltare nel file aperto, barra laterale, apertura rapida, posizione del cursore */
  goto = $state<{ path: string; line: number; col: number; n: number } | null>(null);
  sidebar = $state<"files" | "search" | "problems">("files");
  quickOpen = $state(false);
  cursor = $state({ line: 1, col: 1, cursors: 1 });
  /** Testo selezionato nell'editor (va a Claude con il messaggio, come il contesto di un IDE) */
  selection = $state<{ text: string; from: number; to: number } | null>(null);
  /** Ricerca nel progetto (anche aperta da «vai alla definizione» con più risultati) */
  searchQuery = $state("");
  searchRegex = $state(false);
  /** Problemi: errori e avvisi dei compilatori del progetto */
  problems = $state<IdeProblem[]>([]);
  checkTools = $state<string[]>([]);
  checking = $state(false);
  checkedAt = $state<number | null>(null);
  private checkTimer: ReturnType<typeof setTimeout> | undefined;

  current = $derived(this.files.find((f) => f.path === this.active) ?? null);
  lastTask = $derived(this.tasks.at(-1) ?? null);
  /** Claude (o il CTO) lavora, o c'è un piano da approvare: i nuovi messaggi aspettano */
  busy = $derived(!!this.dev.session?.running || ["deciding", "planning", "working", "plan-ready", "reviewing"].includes(this.tasks.at(-1)?.phase ?? ""));
  activeAgents = $derived(this.team?.agents.filter((a) => a.running).length ?? 0);

  constructor(project: StudioProject) {
    this.project = project;
    this.git = new GitStore(this.dev);
  }

  get path() {
    return this.project.path;
  }

  async start() {
    await this.dev.open(this.project.path);
    await this.restore();
    this.dev.start();
    this.dev.watch();
    this.git.start(this.project.path);
    this.refreshTeam();
    this.timers.push(setInterval(() => this.refreshTeam(), 2000));
    this.permissions = await api.studioPermissions(this.path).catch(() => []);
    this.unlisten.push(await on("studio-permission", (p) => p.folder === this.path && !this.permissions.some((x) => x.id === p.id) && (this.permissions = [...this.permissions, p])));
    this.unlisten.push(await on("studio-permission-done", (p) => (this.permissions = this.permissions.filter((x) => x.id !== p.id))));
    api.studioAgents(this.path).then((a) => (this.agentDefs = a)).catch(() => {});
  }

  stop() {
    for (const t of this.timers) clearInterval(t);
    this.timers = [];
    for (const u of this.unlisten) u();
    this.unlisten = [];
    this.dev.stop();
    this.git.stop();
  }

  /** Ultima lettura della squadra (per capire se la vista è aggiornata) */
  teamAt = $state<number | null>(null);
  teamError = $state<string | null>(null);

  async refreshTeam() {
    // Conversazione nuova: niente squadra finché non parte la prima richiesta
    if (this.fresh) {
      this.team = null;
      return;
    }
    const prima = this.activeAgents;
    try {
      this.team = await api.studioTeam(this.path, this.dev.sessionId ?? this.dev.session?.id);
      this.teamError = null;
    } catch (e) {
      this.teamError = errorText(e);
    }
    this.teamAt = Date.now();
    // Appena il senior divide il lavoro, se non stai guardando un file, la Squadra si apre da sola
    if (!prima && this.activeAgents && this.dev.tab === "file" && !this.files.length) this.dev.tab = "team";
  }

  // ---------- Editor ----------

  async openFile(path: string, line?: number, col = 1) {
    if (line) this.goto = { path, line, col, n: (this.goto?.n ?? 0) + 1 };
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
      // Dopo il primo controllo, ogni salvataggio lo ripete (con calma: i compilatori sono lenti)
      if (this.checkedAt) {
        clearTimeout(this.checkTimer);
        this.checkTimer = setTimeout(() => this.runCheck(), 1200);
      }
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

  async send(text: string, newSession = false, files: string[] = [], follow = false) {
    if (!text.trim() && !files.length) return;
    if (!text.trim()) text = files.length === 1 ? "Guarda il file allegato." : "Guarda i file allegati.";
    // Un piano scartato o un lavoro fermato restano nella sessione di Claude: glielo si dice, così non li riprende
    const prec = this.tasks.at(-1);
    const annullato = !!prec && ((prec.phase === "done" && prec.dropped) || (prec.phase === "error" && prec.error === "Fermato da te"));
    // Seguito di un lavoro appena concluso: stessa sessione, stesso modello, niente nuovo piano (salvo «Sempre piano»).
    // Vale per i messaggi accodati e per le risposte brevi («ok fallo», «sì», «procedi»…) nella stessa conversazione.
    const inConversazione = !newSession && !this.fresh && !!prec && !prec.dropped && !!prec.session && ["done", "review-ready", "error"].includes(prec.phase);
    const breve = /^(ok|okay|si|sì|va bene|vai|procedi|fallo|fallo tu|fai tu|esegui|continua|perfetto|certo|confermo|approvo|go|yes)\b/i.test(text.trim()) || text.trim().split(/\s+/).length <= 4;
    // Dopo le proposte grafiche qualunque risposta è la scelta (o la richiesta di altre): stessa sessione
    const seguito = inConversazione && (follow || breve || !!prec.proposals);
    const task: Task = { id: ++this.taskId, text: text.trim(), decision: null, model: seguito && this.modelChoice === "auto" ? prec.model : this.modelChoice, plan: this.planChoice === "yes", profile: this.profile, session: null, phase: "deciding", fixRounds: 0, files };
    this.tasks = [...this.tasks, task];
    const invio = annullato
      ? `[L'utente ha annullato il piano/lavoro precedente: non proseguirlo e non darlo per approvato. Questa è la nuova richiesta, da valutare da capo.]\n${task.text}`
      : seguito && prec.proposals
        ? `${SCELTA}\n${task.text}`
        : seguito
        ? `[Seguito del lavoro appena concluso in questa stessa sessione: il piano era già fatto ed eseguito, non rifarlo; integra quanto segue e verifica.]\n${task.text}`
        : task.text;
    // Come in un IDE: Claude sa quale file hai davanti e cosa hai selezionato («sistema questo», «questa funzione»)
    const contestoEditor = this.contestoEditor();
    let daInviare = contestoEditor ? `${invio}\n\n${contestoEditor}` : invio;
    const t = this.tasks[this.tasks.length - 1];
    try {
      if (seguito) {
        t.decision = { kind: "media", model: t.model as StudioDecision["model"], plan: t.plan, reason: "Seguito del lavoro precedente: stessa sessione, senza nuovo piano", source: "coda" };
      } else if (this.modelChoice === "auto" || this.planChoice === "auto") {
        // L'orchestratore decide modello e piano se l'utente non ha scelto
        t.decision = await api.studioClassify(this.path, t.text, inConversazione ? this.contesto(prec!) : undefined);
        if (this.modelChoice === "auto") t.model = t.decision.model;
        if (this.planChoice === "auto") t.plan = t.decision.plan;
      }
      // Lavoro visivo nuovo: si parte sempre dalle proposte, senza piano (il piano viene dopo la scelta)
      if (t.decision?.grafica && !seguito) {
        t.plan = false;
        daInviare = `${PROPOSTE}\n${daInviare}`;
      }
      t.phase = t.plan ? "planning" : "working";
      const nuova = newSession || this.fresh;
      this.fresh = false;
      const r = await api.studioRun(this.path, daInviare, t.model, t.plan, t.profile || undefined, nuova ? undefined : (this.dev.sessionId ?? this.dev.session?.id ?? undefined), nuova, files);
      t.session = r.session;
      this.dev.sessionId = r.session || null;
      this.dev.refreshSession();
      this.persist();
    } catch (e) {
      t.phase = "error";
      t.error = errorText(e);
      this.drain();
    }
  }

  private contestoEditor() {
    const f = this.current;
    if (!f || this.dev.tab !== "file") return "";
    const rel = f.path.startsWith(this.path) ? f.path.slice(this.path.length + 1) : f.path;
    const sel = this.selection;
    if (sel?.text.trim()) {
      const lang = f.name.split(".").pop() ?? "";
      return `<contesto_editor>Nell'editor l'utente ha aperto ${rel} e ha selezionato le righe ${sel.from}-${sel.to}:\n\`\`\`${lang}\n${sel.text}\n\`\`\`\n«Questo», «qui» e simili si riferiscono probabilmente alla selezione.</contesto_editor>`;
    }
    return `<contesto_editor>Nell'editor l'utente ha aperto ${rel}, con il cursore alla riga ${this.cursor.line}.</contesto_editor>`;
  }

  /** Il filo della conversazione per l'orchestratore: la richiesta precedente e l'ultima risposta di Claude. */
  private contesto(prec: Task) {
    const ultima = [...(this.dev.session?.items ?? [])].reverse().find((i) => i.kind === "text")?.text ?? "";
    return `Richiesta precedente: ${prec.text.slice(0, 600)}\nUltima risposta di Claude: ${ultima.slice(0, 1200)}`;
  }

  /** Compiti e conversazione salvati per progetto: nelle preferenze dell'app (durano ai riavvii) e nel webview. */
  persist() {
    const stato = { tasks: this.tasks.slice(-30), fresh: this.fresh, session: this.dev.sessionId };
    try {
      localStorage.setItem(`oltremare:studio-tasks:${this.path}`, JSON.stringify(stato));
    } catch {}
    api.prefsSet(`studio:stato:${this.path}`, stato).catch(() => {});
  }

  private async restore() {
    type Salvato = { tasks: Task[]; fresh?: boolean; session?: string | null };
    const saved = (await api.prefsGet<Salvato>(`studio:stato:${this.path}`).catch(() => null)) ?? load<Salvato | null>(`oltremare:studio-tasks:${this.path}`, null);
    // Ciò che era a metà quando lo Studio si è chiuso non può riprendere: si mostra come concluso
    this.tasks = (saved?.tasks ?? []).map((t) => (["deciding", "planning", "working", "reviewing"].includes(t.phase) ? { ...t, phase: "done" as const } : t));
    this.fresh = !!saved?.fresh;
    this.taskId = Math.max(0, ...this.tasks.map((t) => t.id));
    const sid = saved?.session ?? [...this.tasks].reverse().find((t) => t.session)?.session;
    if (sid && !this.fresh) this.dev.sessionId = sid;
    this.refreshMemory();
  }

  /** Riprende una conversazione passata del progetto: la vista e la prossima richiesta continuano da lì. */
  switchSession(id: string) {
    this.dev.sessionId = id;
    this.fresh = false;
    this.dev.refreshSession();
    this.persist();
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
    this.team = null;
    this.persist();
  }

  enqueue(text: string, files: string[] = []) {
    // Scritto mentre Claude lavora: è un seguito di quel lavoro, non una richiesta da ripianificare
    const follow = ["working", "reviewing"].includes(this.lastTask?.phase ?? "");
    if (text.trim() || files.length) this.queue = [...this.queue, { id: ++this.queueId, text: text.trim(), files, follow }];
  }

  unqueue(id: number) {
    this.queue = this.queue.filter((q) => q.id !== id);
  }

  /** Manda il primo messaggio in coda, se Claude è libero. */
  drain() {
    if (this.busy || !this.queue.length) return;
    const [q, ...rest] = this.queue;
    this.queue = rest;
    this.send(q.text, false, q.files, q.follow);
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
    if (!t || t.phase === "error" || t.phase === "done") {
      this.drain();
      return;
    }
    if (t.phase === "planning" && t.session) {
      const r = await api.studioPlan(this.path, t.session).catch(() => ({ plan: "" }));
      t.planText = r.plan;
      t.phase = "plan-ready";
    } else if (t.phase === "working") {
      this.reloadClean();
      // La nota del progetto in memoria nasce (o si aggiorna) a fine lavoro
      this.refreshMemory();
      // Fermo sulle proposte: niente revisione, si aspetta la scelta dell'utente
      const fine = [...(this.dev.session?.items ?? [])].reverse().find((i) => i.kind === "text")?.text ?? "";
      t.proposals = fine.includes(".oltremare/proposte/");
      if (t.proposals) t.phase = "done";
      else if (await this.wantsReview(t)) await this.startReview(t);
      else t.phase = "done";
    } else if (t.phase === "reviewing" && t.reviewSession) {
      const r = (await api.studioReviewResult(this.path, t.reviewSession).catch(() => null)) ?? { esito: "sconosciuto" as const, sintesi: "", problemi: [], voto: null, testo: "Non sono riuscito a leggere il verdetto." };
      const reviews = [...(t.reviews ?? [])];
      const second = secondReviewer(t.profile);
      const who = reviews.length ? `Secondo revisore · ${second?.name ?? "altra AI"}` : `CTO · ${this.reviewer(t) === "fable" ? "Fable" : "Opus"}`;
      reviews.push({ who, profile: reviews.length ? (second?.name ?? "") : "", review: r });
      t.reviews = reviews;
      t.reviewSession = null;
      // Con «Doppia revisione», dopo il CTO rivede anche un'altra AI (una sola volta per tornata)
      if (reviews.length === 1 && second && this.reviewChoice === "yes") {
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
    // In automatico il CTO rivede i lavori grandi e le modifiche estese; il resto lo verifica già il senior
    return t.decision?.kind === "grande" || cambiati >= 6;
  }

  /** Il CTO rivede con Sonnet (rapido); con Fable resta Fable */
  private reviewer(t: Task) {
    return t.model === "fable" ? "fable" : "sonnet";
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

  /** Scarta il piano: non si esegue; la conversazione resta e la coda riparte. */
  discard(t: Task) {
    if (t.phase !== "plan-ready") return;
    t.dropped = true;
    t.phase = "done";
    this.drain();
  }

  /** Ferma Claude (o il CTO) su questo compito e lo chiude come interrotto. */
  async halt(t: Task) {
    if (this.dev.session?.running) await this.dev.stopClaude();
    if (["planning", "working", "reviewing"].includes(t.phase)) {
      t.phase = "error";
      t.error = "Fermato da te";
    }
    this.drain();
  }

  async answerPermission(id: string, allow: boolean) {
    this.permissions = this.permissions.filter((x) => x.id !== id);
    await api.studioPermissionAnswer(id, allow).catch((e) => ui.toast(errorText(e), "error"));
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

  // ---------- Problemi e definizioni ----------

  async runCheck() {
    if (this.checking) return;
    this.checking = true;
    try {
      const r = await api.ideCheck(this.path);
      this.problems = r.problems;
      this.checkTools = r.tools;
      this.checkedAt = Date.now();
    } catch (e) {
      ui.toast(errorText(e), "error");
    } finally {
      this.checking = false;
    }
  }

  /** F12 / ⌘+clic: una definizione → la apre; più d'una → le mostra nella ricerca */
  async goToDefinition(name: string) {
    const r = await api.ideDefinition(this.path, name).catch(() => []);
    if (r.length === 1) this.openFile(r[0].path, r[0].line, r[0].col);
    else if (!r.length) ui.toast(`Nessuna definizione trovata per «${name}»`);
    else {
      const first = r[0];
      if (/\b(function|class|interface|type|enum|fn|struct|trait|def)\b/.test(first.text) && !/\b(function|class|interface|type|enum|fn|struct|trait|def)\b/.test(r[1].text)) this.openFile(first.path, first.line, first.col);
      else {
        this.searchRegex = false;
        this.searchQuery = name;
        this.sidebar = "search";
      }
    }
  }

  // ---------- File (menu dell'albero) ----------

  private async ricarica(p: string) {
    const madre = p.slice(0, Math.max(p.lastIndexOf("/"), 0)) || this.path;
    await this.dev.loadDir(madre);
    this.dev.refreshGit();
  }

  async createEntry(dir: string, name: string, isDir: boolean) {
    try {
      const p = await api.ideCreate(this.path, `${dir}/${name.trim()}`, isDir);
      this.dev.expanded[dir] = true;
      await this.ricarica(p);
      if (!isDir) this.openFile(p);
    } catch (e) {
      ui.toast(errorText(e), "error");
    }
  }

  async renameEntry(from: string, name: string) {
    try {
      const to = await api.ideRename(this.path, from, `${from.slice(0, from.lastIndexOf("/"))}/${name.trim()}`);
      // I file aperti seguono il nuovo nome
      for (const f of this.files) {
        if (f.path === from || f.path.startsWith(from + "/")) {
          const np = to + f.path.slice(from.length);
          if (this.active === f.path) this.active = np;
          f.path = np;
          f.name = np.split("/").pop() ?? np;
        }
      }
      await this.ricarica(to);
    } catch (e) {
      ui.toast(errorText(e), "error");
    }
  }

  async trashEntry(p: string) {
    try {
      await api.ideTrash(this.path, p);
      for (const f of [...this.files]) if (f.path === p || f.path.startsWith(p + "/")) this.closeFile(f.path);
      await this.ricarica(p);
      ui.toast("Spostato nel Cestino");
    } catch (e) {
      ui.toast(errorText(e), "error");
    }
  }
}
