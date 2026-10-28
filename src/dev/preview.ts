// Anteprima nel browser (solo `npm run dev` fuori da Tauri): dati finti per lavorare sulla grafica.
// Simula streaming, passi e stati dell'avatar. Non entra mai nella build di produzione.
// Scenari: ?scenario=default | empty | error | dark
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import type { Agent, Conversation, Run, Settings, Step } from "../lib/api";

const scenario = new URLSearchParams(location.search).get("scenario") ?? "default";
const now = Date.now();
const min = 60_000;
const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));
const uid = () => Math.random().toString(36).slice(2, 10);

const policy: Agent["policy"] = { readDefault: "allow", writeDefault: "ask", dangerDefault: "ask", perTool: {}, approvalTimeoutS: 180, allowSession: true };
const base: Omit<Agent, "id" | "name" | "icon" | "hue" | "description" | "systemPrompt" | "provider" | "model" | "system"> = {
  effort: "medium", maxTokens: 32000, maxTurns: 40, tools: [], policy, allowedPaths: [], mailAccounts: [], browserDomains: [],
  voiceReply: "whenAsked", triggers: [], createdAt: now - 30 * 86400000, updatedAt: now,
};

let agents: Agent[] = [
  { ...base, id: "assistente", name: "Assistente", icon: "spark", hue: 275, description: "Il tuo assistente personale", systemPrompt: "Sei Oltremare…", provider: "anthropic", model: "claude-opus-5-5", system: true, tools: ["fs", "shell", "apps", "clipboard"] },
  { ...base, id: "organizzatore", name: "Organizzatore", icon: "calendar", hue: 205, description: "Pianifica giornate e settimane", systemPrompt: "Sei il mio organizzatore.", provider: "anthropic", model: "claude-sonnet-5-5", system: false },
  { ...base, id: "ricercatore", name: "Ricercatore", icon: "globe", hue: 160, description: "Cerca, confronta, riassume", systemPrompt: "Sei un ricercatore meticoloso.", provider: "openai", model: "gpt-5", effort: "", system: false },
  { ...base, id: "scrittore", name: "Scrittore", icon: "pen", hue: 330, description: "Mail, testi e correzioni", systemPrompt: "Scrivi in modo chiaro.", provider: "claude-code", model: "default", system: false },
];

let settings: Settings = {
  userName: "Max", language: "italiano", anthropicModel: "claude-opus-5-5", openaiModel: "gpt-5", openaiBaseUrl: "https://api.openai.com/v1",
  claudeCodePath: "", claudeCodeModel: "default", localModel: "gemma-4-26b-a4b",
  theme: scenario === "dark" ? "dark" : "system", avatarFace: "character", sendWithEnter: true, showThinking: true,
  voiceListen: false, voiceStt: "whisper-small", voiceTts: "piper-paola", voiceSpeed: 1, voiceDevice: "", voiceAgent: "assistente", voiceHotkey: "Alt+Shift+Space", companion: false, msClientId: "", googleClientId: "", googleClientSecret: "",
  notificationsSystem: false, notificationsTeams: true, notificationsSlack: false, notificationsMail: true, notificationsLocalOnly: false,
  notificationRules: [{ id: "r1", app: "Teams", contains: "urgente", action: "speak", agentId: "", enabled: true }],
  quietFrom: "22:00", quietTo: "07:30", briefingEnabled: true, briefingTime: "08:00", briefingDays: [1, 2, 3, 4, 5],
  accentHue: 275, accentChroma: 0.2, density: "comfortable", fontScale: 1, paletteHotkey: "Alt+Space", autostart: false, layout: "operator",
  mcpServers: [{ id: "m1", name: "issue", kind: "http", url: "https://issue.example.com/api/mcp", headersHelper: 'cat "$HOME/.issue/headers.json"', command: "", args: [], enabled: true }],
  editorApp: "", devMode: "editor", companionStyle: "island", browserVisible: false, computerWaitIdle: true,
};
const keys = { anthropic: scenario !== "empty", openai: scenario !== "empty" };

const step = (kind: Step["kind"], text: string, at: number, len = 0): Step => ({ id: uid(), kind, text, at, endedAt: at + len });

const conversations: Conversation[] = scenario === "empty" ? [] : [
  { id: "c1", agentId: "assistente", title: "Idee per il weekend a Torino", createdAt: now - 2 * 3600_000, updatedAt: now - 90 * min },
  { id: "c2", agentId: "assistente", title: "Differenza tra TFR in azienda e fondo pensione", createdAt: now - 26 * 3600_000, updatedAt: now - 25 * 3600_000 },
  { id: "c3", agentId: "assistente", title: "Riscrivi la mail per il condominio", createdAt: now - 3 * 86400_000, updatedAt: now - 3 * 86400_000 },
  { id: "c4", agentId: "organizzatore", title: "Settimana del trasloco", createdAt: now - 86400_000, updatedAt: now - 86400_000 },
];

const runsByConv: Record<string, Run[]> = {
  c1: [
    {
      id: "r1", agentId: "assistente", conversationId: "c1", title: "Idee per il weekend a Torino", trigger: "main", status: "done",
      startedAt: now - 92 * min, endedAt: now - 91.6 * min, error: null, costEstimate: 0.0187, servedModel: "claude-opus-5-5",
      usage: { inputTokens: 412, outputTokens: 610, cacheRead: 1830, cacheWrite: 0 },
      steps: [
        step("user", "Sabato e domenica a Torino con due bambini di 6 e 9 anni: cosa mi consigli?", now - 92 * min),
        step("thinking", "Famiglia con bambini piccoli: musei interattivi, parchi, pause frequenti. Torino ha il Museo del Cinema, l'Egizio con percorsi per ragazzi, il Parco del Valentino.", now - 91.95 * min, 3800),
        step("text", "Ecco un weekend a misura di bambini, senza corse:\n\n**Sabato**\n- **Museo Egizio** al mattino: c'è un percorso pensato per ragazzi, chiedi la mappa \"Junior\" alla cassa.\n- Pranzo veloce in **piazza Castello** e passeggiata sotto i portici di via Po.\n- Pomeriggio al **Parco del Valentino**, con il Borgo Medievale.\n\n**Domenica**\n- **Museo del Cinema** nella Mole: l'ascensore panoramico piace sempre.\n- Merenda con un *bicerin* (per voi) e una cioccolata (per loro).\n\nVuoi che prepari anche gli orari e i biglietti da prenotare?", now - 91.9 * min, 18000),
      ],
    },
  ],
};

let streaming: Record<string, boolean> = {};
const waiting = new Map<string, (d: string) => void>();
const catalog = [
  ["leggi_file", "Leggere file", "fs", "read"], ["elenca_cartella", "Elencare cartelle", "fs", "read"], ["cerca_file", "Cercare file", "fs", "read"],
  ["scrivi_file", "Scrivere file", "fs", "write"], ["sposta_file", "Spostare e rinominare", "fs", "write"], ["cestina_file", "Spostare nel Cestino", "fs", "write"],
  ["esegui_comando", "Eseguire comandi", "shell", "danger"], ["apri_app", "Aprire app", "apps", "write"], ["leggi_appunti", "Leggere gli appunti", "clipboard", "read"],
].map(([name, label, category, risk]) => ({ name, label, category, risk, description: `Descrizione di ${label.toLowerCase()}`, defaultPolicy: risk === "read" ? "allow" : "ask" }));

async function toolStep(run: Run, step: any) {
  await emit("run-step", { runId: run.id, step });
}

async function simulate(run: Run, text: string) {
  const say = (state: string, level = 0) => emit("avatar-state", { state, level });
  await wait(250);
  say("thinking");
  const think = "L'utente chiede: «" + text.slice(0, 60) + "». Rispondo in modo concreto, con un elenco breve e una domanda finale per proseguire.";
  const tid = uid();
  for (const w of think.split(/(\s+)/)) {
    if (!streaming[run.id]) return;
    await emit("run-delta", { runId: run.id, stepId: tid, kind: "thinking", delta: w });
    await wait(18);
  }
  const t0 = Date.now();
  await emit("run-step", { runId: run.id, step: { id: tid, kind: "thinking", text: think, at: t0 - 1500, endedAt: t0 } });
  // Uno strumento di lettura, poi uno che chiede approvazione
  await emit("avatar-state", { state: "working", level: 0 });
  const s1 = { id: uid(), kind: "tool", text: "", at: Date.now(), tool: "elenca_cartella", category: "fs", title: "Guarda dentro Documents", input: { percorso: "~/Documents" }, status: "running" };
  await toolStep(run, s1);
  await wait(700);
  await toolStep(run, { ...s1, status: "done", endedAt: Date.now(), result: { kind: "fs", op: "list", path: "~/Documents", entries: ["Preventivi/", "Fatture/", "note.txt  (2 KB, 21/10/2026 18:02)", "piano-trasloco.md  (4 KB, 25/10/2026 09:11)"] } });
  const t2 = { id: uid(), kind: "tool", text: "", at: Date.now(), tool: "scrivi_file", category: "fs", title: "Scrive riepilogo.md", input: { percorso: "~/Documents/riepilogo.md", contenuto: "# Riepilogo\n- Preventivi da rivedere: 2\n" }, status: "awaitingApproval" };
  await toolStep(run, t2);
  const approvalId = uid();
  await emit("avatar-state", { state: "asking", level: 0 });
  await emit("run-status", { runId: run.id, status: "awaitingApproval" });
  await emit("run-approval", { runId: run.id, request: {
    id: approvalId, runId: run.id, stepId: t2.id, agentId: run.agentId, agentName: "Assistente", tool: "scrivi_file", category: "fs", risk: "write",
    title: "Scrive riepilogo.md", summary: "Nuovo file ~/Documents/riepilogo.md (2 righe)",
    input: t2.input, diff: "--- prima\n+++ dopo\n@@ -0,0 +1,2 @@\n+# Riepilogo\n+- Preventivi da rivedere: 2", expiresAt: Date.now() + 120_000, allowSession: true,
  } });
  const decision = await new Promise<string>((res) => waiting.set(approvalId, res));
  await emit("run-approval-resolved", { approvalId, runId: run.id, decision, by: "user" });
  await emit("run-status", { runId: run.id, status: "running" });
  if (decision === "deny") {
    await toolStep(run, { ...t2, status: "denied", endedAt: Date.now(), error: "L'utente ha negato l'azione" });
  } else {
    await emit("avatar-state", { state: "working", level: 0 });
    await wait(400);
    await toolStep(run, { ...t2, status: "done", endedAt: Date.now(), result: { kind: "fs", op: "write", path: "~/Documents/riepilogo.md", bytes: 42, diff: "--- prima\n+++ dopo\n@@ -0,0 +1,2 @@\n+# Riepilogo\n+- Preventivi da rivedere: 2" } });
  }
  await emit("avatar-state", { state: "thinking", level: 0 });
  if (scenario === "error") {
    await wait(400);
    await emit("avatar-state", { state: "error", level: 0 });
    await emit("run-done", { runId: run.id, conversationId: run.conversationId, status: "error", usage: { inputTokens: 0, outputTokens: 0, cacheRead: 0, cacheWrite: 0 }, costEstimate: null, servedModel: null, error: "invalid x-api-key — controlla la chiave API nelle impostazioni", endedAt: Date.now() });
    setTimeout(() => say("idle"), 2600);
    return;
  }
  say("speaking");
  const answer =
    "Certo! Ecco come la vedo:\n\n1. **Parti dalle priorità**: scrivi le tre cose che contano davvero oggi.\n2. **Blocca il tempo**: dedica a ciascuna una fascia precisa in calendario.\n3. **Lascia margine**: tieni libera mezz'ora per gli imprevisti.\n\nSe vuoi, quando arriveranno gli strumenti potrò anche *inserire gli impegni nel tuo calendario* per te. Da dove partiamo?";
  const sid = uid();
  for (const w of answer.split(/(\s+)/)) {
    if (!streaming[run.id]) break;
    await emit("run-delta", { runId: run.id, stepId: sid, kind: "text", delta: w });
    await wait(28 + Math.random() * 40);
  }
  const t1 = Date.now();
  await emit("run-step", { runId: run.id, step: { id: sid, kind: "text", text: answer, at: t0, endedAt: t1 } });
  const cancelled = !streaming[run.id];
  delete streaming[run.id];
  say("idle");
  await emit("run-done", {
    runId: run.id, conversationId: run.conversationId, status: cancelled ? "cancelled" : "done",
    usage: { inputTokens: 380, outputTokens: 240, cacheRead: 2100, cacheWrite: 0 }, costEstimate: 0.0074, servedModel: "claude-opus-5-5", error: null, endedAt: t1,
  });
}

export function installPreview() {
  mockWindows("main");
  mockIPC(
    async (cmd, args) => {
      const a = (args ?? {}) as Record<string, any>;
      switch (cmd) {
        case "settings_get":
          return { settings, keys };
        case "settings_save":
          settings = a.settings;
          return null;
        case "provider_key_set":
          keys[a.provider as "anthropic"] = !!a.key;
          return null;
        case "provider_models":
          await wait(300);
          return a.provider === "anthropic" ? ["claude-opus-5-5", "claude-sonnet-5-5", "claude-haiku-5-5", "claude-fable-5-1"] : ["gpt-5", "gpt-5-mini"];
        case "provider_test":
          await wait(500);
          return { ok: true, ms: 312, models: 4, error: null };
        case "prefs_get":
          return null;
        case "prefs_set":
          return null;
        case "agents_list":
          return agents;
        case "agent_save": {
          const ag = { ...a.agent, id: a.agent.id || uid(), updatedAt: Date.now(), createdAt: a.agent.createdAt || Date.now() };
          agents = agents.some((x) => x.id === ag.id) ? agents.map((x) => (x.id === ag.id ? ag : x)) : [...agents, ag];
          return ag;
        }
        case "agent_delete":
          agents = agents.filter((x) => x.id !== a.id);
          return null;
        case "agent_duplicate": {
          const src = agents.find((x) => x.id === a.id)!;
          const copy = { ...src, id: uid(), name: `${src.name} (copia)`, system: false };
          agents = [...agents, copy];
          return copy;
        }
        case "conversations_list":
          await wait(120);
          return conversations.filter((c) => c.agentId === a.agentId);
        case "conversation_runs":
          await wait(250);
          return runsByConv[a.conversationId] ?? [];
        case "conversation_delete":
          return null;
        case "avatar_state":
          return "idle";
        case "llama_status":
          return { state: "stopped", port: null, build: "b11577", backend: "metal", model: null, message: null, installed: true };
        case "hw_info":
          return { ramTotal: 34_359_738_368, ramAvailable: 18_000_000_000, cpu: "Apple M5", backend: "metal", os: "macos" };
        case "models_catalog":
          return [
            { id: "gemma-4-e4b", kind: "llm", label: "Gemma 4 E4B", note: "Piccolo e veloce, vede le immagini", repo: "", sizeBytes: 5_970_000_000, ramNeededGb: 8, ctx: 32768, supportsTools: true, supportsVision: true, recommended: false, installed: false, custom: false },
            { id: "qwen3.5-4b", kind: "llm", label: "Qwen 3.5 4B", note: "Il più leggero", repo: "", sizeBytes: 3_410_000_000, ramNeededGb: 6, ctx: 32768, supportsTools: true, supportsVision: true, recommended: false, installed: true, custom: false },
            { id: "gemma-4-26b-a4b", kind: "llm", label: "Gemma 4 26B", note: "Consigliato con 32 GB", repo: "", sizeBytes: 18_140_000_000, ramNeededGb: 22, ctx: 32768, supportsTools: true, supportsVision: true, recommended: true, installed: false, custom: false },
          ];
        case "model_download":
          return null;
        case "voice_status":
          return { wakeListening: false, active: false, speaking: false, models: { vad: true, stt: true, tts: true } };
        case "voice_devices":
          return ["Microfono MacBook Pro", "AirPods Pro"];
        case "voice_command":
          setTimeout(() => emit("voice-status", { listening: false, recording: true, error: null }), 50);
          setTimeout(() => emit("avatar-state", { state: "listening", level: 0.5 }), 80);
          setTimeout(() => emit("voice-status", { listening: false, recording: false, error: null }), 2500);
          setTimeout(() => emit("voice-transcript", { text: "che tempo farà domani a Milano?", final: true }), 2600);
          setTimeout(() => emit("avatar-state", { state: "idle", level: 0 }), 2600);
          return null;
        case "notifications_list":
          return [
            { id: "n1", at: now - 12 * min, app: "Teams", appId: "teams", title: "Giulia Neri", body: "Ci vediamo alle 15 per la revisione? Porto io i numeri aggiornati.", source: "teams", read: false },
            { id: "n2", at: now - 40 * min, app: "Posta · max@contoso.example", appId: "mail", title: "Studio Rossi: Preventivo 2027", body: "In allegato il preventivo aggiornato con le voci richieste.", source: "mail", read: false },
            { id: "n3", at: now - 3 * 3600_000, app: "WhatsApp", appId: "net.whatsapp.WhatsApp", title: "Mamma", body: "Domenica pranzo da noi?", source: "system", read: true },
          ];
        case "notifications_sources":
          return { systemSupported: true, fullDisk: false, teamsAccounts: [], slack: null };
        case "briefing_latest":
          return { date: new Date().toISOString().slice(0, 10), runId: "r1", conversationId: "c1" };
        case "cron_preview":
          return [now + 3600_000, now + 86400_000, now + 2 * 86400_000];
        case "hotkey_check":
          return null;
        case "usage_stats": {
          const daily = Array.from({ length: 30 }, (_, i) => ({ runs: i % 4, tokens: (i % 7) * 42000 + (i > 24 ? 160000 : 0), cost: 0, value: (i % 7) * 0.4 }));
          const dailyApi = Array.from({ length: 30 }, (_, i) => ({ runs: i % 3, tokens: (i % 5) * 9000, cost: (i % 5) * 0.11, value: 0 }));
          return {
            since: now - 29 * 86400_000, days: 30,
            subscription: { status: "allowed", resetsAt: Math.floor(now / 1000) + 7200, rateLimitType: "five_hour", overageStatus: "rejected", isUsingOverage: false },
            providers: {
              "claude-code": { today: { runs: 42, input: 3029143, output: 47408, value: 18.4 }, week: { runs: 120, input: 8100000, output: 140000, value: 51.2 }, month: { runs: 260, input: 17000000, output: 310000, value: 112.7 }, daily, models: { "claude-opus-5": { runs: 240 }, "claude-sonnet-5": { runs: 20 } }, agents: { assistente: { runs: 150, input: 9000000 }, ricercatore: { runs: 80, input: 6000000 } } },
              openai: { today: { runs: 3, input: 21000, output: 4000, cost: 0.34 }, week: { runs: 18, input: 120000, output: 30000, cost: 2.1 }, month: { runs: 50, input: 400000, output: 90000, cost: 6.8 }, daily: dailyApi, models: { "gpt-5": { runs: 50, cost: 6.8 } }, agents: { ricercatore: { runs: 50, cost: 6.8 } } },
            },
          };
        }
        case "island_info":
          return { present: true, width: 186, height: 32 };
        case "studio_projects": {
          const P = [
            ["Oltremare", "Suites", "Personale", ["Tauri", "Svelte", "TypeScript", "Rust"], "main", 3, 275, "Assistente personale desktop con agenti, voce e avatar."],
            ["Cobalto", "Suites", "Personale", ["Tauri", "Svelte", "Rust"], "main", 0, 230, "Client di posta desktop blu e bianco."],
            ["KeyOps", "", "Sviluppo", ["Next.js", "TypeScript", "Prisma", "Docker"], "develop", 12, 300, "Gestionale di issue e progetti."],
            ["FarmaQuest", "", "Sviluppo", ["React", "TypeScript", "Docker", "Node"], "main", 0, 150, "Piattaforma per farmacie con backend e pannello admin."],
            ["WeddingPlan", "", "Personale", ["Next.js", "TypeScript"], "main", 1, 340, "Pianificazione del matrimonio: fornitori, budget, invitati."],
            ["SmartEdil", "", "Personale", ["Next.js", "Docker"], "main", 0, 40, "Gestionale per l'edilizia."],
            ["CantierOS", "", "Sviluppo", [], "main", 0, 20, null],
            ["Portfolio", "", "Personale", ["Svelte"], "main", 0, 190, "Sito personale."],
          ];
          return P.map(([name, group, root, stack, branch, changes, hue, readme], i) => ({ name, group, root, path: `/Users/demo/Desktop/${root}/${group ? group + "/" : ""}${name}`, stack, branch, changes, updatedAt: now - i * 5 * 3600_000, sessions: i, lastSession: null, icon: null, hue, readme }));
        }
        case "studio_link":
          return { link: { server: "issue", project: "OLT", name: "Oltremare" }, servers: ["issue"], canImport: true };
        case "studio_issues":
          return { issues: [{ key: "OLT-12", title: "Sconti nel carrello", status: "In corso", extra: null }, { key: "OLT-9", title: "Esportazione PDF delle fatture", status: "Da fare", extra: null }, { key: "OLT-4", title: "Login con Google", status: "In revisione", extra: null }] };
        case "studio_issue":
          return "# OLT-12 Sconti nel carrello\nStato: In corso\n\n## Descrizione\nCodice sconto in percentuale, con test.";
        case "studio_claudex":
          return { installed: false, profiles: [] };
        case "studio_classify":
          await wait(600);
          return { kind: "media", model: "sonnet", plan: true, reason: "Tocca più file del carrello e serve decidere dove calcolare lo sconto.", source: "orchestratore" };
        case "studio_run":
          return { session: "s1", plan: a.plan };
        case "studio_plan":
          return { plan: "## Piano\n1. Aggiungere `applicaSconto` in `src/sconti.ts`\n2. Usarla in `totale()`\n3. Test con Vitest" };
        case "dev_projects":
          return ["/Users/demo/Sviluppo/negozio"];
        case "dev_open":
          return a.folder;
        case "dev_tree": {
          const base = "/Users/demo/Sviluppo/negozio";
          const T: Record<string, [string, boolean][]> = {
            [base]: [["src", true], ["public", true], ["tests", true], ["docs", true], ["scripts", true], [".github", true], ["package.json", false], ["README.md", false], ["tsconfig.json", false], [".env.example", false], ["docker-compose.yml", false], ["pnpm-lock.yaml", false]],
            [base + "/src"]: [["components", true], ["api", true], ["styles", true], ["app.svelte", false], ["carrello.ts", false], ["sconti.ts", false], ["main.ts", false]],
            [base + "/src/components"]: [["Carrello.svelte", false], ["Prodotto.svelte", false], ["Bottone.tsx", false]],
            [base + "/src/api"]: [["ordini.rs", false], ["schema.sql", false]],
            [base + "/src/styles"]: [["tema.css", false], ["reset.scss", false]],
            [base + "/public"]: [["logo.svg", false], ["copertina.png", false], ["index.html", false]],
            [base + "/tests"]: [["carrello.test.ts", false]],
          };
          const key = (a.folder in T) ? a.folder : (a.folder.startsWith(base) ? a.folder : base);
          return (T[key] ?? []).map(([name, dir]) => ({ name, path: `${key}/${name}`, dir }));
        }
        case "dev_file":
          return { path: a.path, content: "export function totale(righe: Riga[]) {\n  return righe.reduce((s, r) => s + r.prezzo * r.qta, 0);\n}\n", binary: false, truncated: false, size: 120, ext: "ts" };
        case "dev_git":
          return { repo: true, branch: "feature/NEG-12-sconti", last: "a1b2c3d Carrello: righe e totale", changes: [{ status: "M", path: "/Users/demo/Sviluppo/negozio/src/carrello.ts", rel: "src/carrello.ts" }, { status: "??", path: "/Users/demo/Sviluppo/negozio/src/sconti.ts", rel: "src/sconti.ts" }, { status: "M", path: "/Users/demo/Sviluppo/negozio/src/components/Carrello.svelte", rel: "src/components/Carrello.svelte" }, { status: "D", path: "/Users/demo/Sviluppo/negozio/src/vecchio.js", rel: "src/vecchio.js" }] };
        case "dev_diff":
          return "diff --git a/src/carrello.ts b/src/carrello.ts\n@@ -1,3 +1,5 @@\n export function totale(righe: Riga[]) {\n-  return righe.reduce((s, r) => s + r.prezzo * r.qta, 0);\n+  const lordo = righe.reduce((s, r) => s + r.prezzo * r.qta, 0);\n+  return applicaSconto(lordo);\n }";
        case "dev_session":
          return { id: "s1", running: false, here: false, stuck: false, sessions: [{ id: "s1", title: "Aggiungi gli sconti al carrello", updatedAt: now }], items: [
            { kind: "user", text: "Aggiungi gli sconti al carrello: codice sconto in percentuale, con test.", at: "" },
            { kind: "tool", tool: "keyops · get_issue", title: "OLT-12", done: true, result: "# OLT-12 Sconti nel carrello", at: "" },
            { kind: "text", text: "Ho letto la issue: guardo come è fatto il carrello.", at: "" },
            { kind: "tool", tool: "Read", title: "src/carrello.ts", done: true, result: "export function totale…", at: "" },
            { kind: "tool", tool: "Grep", title: "applicaSconto", done: true, result: "nessun risultato", at: "" },
            { kind: "tool", tool: "Write", title: "src/sconti.ts", done: true, at: "" },
            { kind: "tool", tool: "Edit", title: "src/components/Carrello.svelte", done: true, at: "" },
            { kind: "tool", tool: "WebSearch", title: "vitest percentuale arrotondamento", done: true, at: "" },
            { kind: "tool", tool: "Bash", title: "npm test -- carrello", done: false, at: "" },
          ] };
        case "dev_processes":
          return [{ id: "p1", name: "frontend", command: "npm run dev", folder: "/Users/demo/Sviluppo/negozio", running: true, exitCode: null, urls: ["http://localhost:5173/"], log: "VITE v8.3.5  ready in 312 ms\n\n  ➜  Local:   http://localhost:5173/" }];
        case "mcp_status":
          return [{ id: "m1", name: "issue", connected: true, connecting: false, tools: ["issue_list_my_issues", "issue_start_work"], error: null }];
        case "mcp_secret_has":
          return { headers: false, env: false };
        case "editors_list":
          return [{ name: "Visual Studio Code", path: "/Applications/Visual Studio Code.app" }];
        case "accounts_list":
          return [{ id: "a1", provider: "microsoft", email: "max@contoso.example", name: "Max", createdAt: now, teams: false }];
        case "oauth_available":
          return ["microsoft"];
        case "tool_catalog":
          return catalog;
        case "approvals_pending":
          return [];
        case "approval_answer":
          waiting.get(a.approvalId)?.(a.decision);
          waiting.delete(a.approvalId);
          return null;
        case "audit_list":
          return [{ at: Date.now() - 600_000, runId: "r1", agentId: "assistente", tool: "elenca_cartella", input: {}, decision: "allow", by: "policy", ok: true, error: null }];
        case "runs_list":
          return Object.values(runsByConv).flat();
        case "attachment_types":
          return ["pdf", "png", "jpg", "jpeg", "xlsx", "xls", "csv", "docx", "pptx", "xml", "json", "txt", "md"];
        case "plugin:dialog|open":
          return ["/Users/demo/Documents/Preventivo 2027.pdf", "/Users/demo/Documents/listino.xlsx"];
        case "claude_code_status":
          await wait(400);
          return scenario === "empty"
            ? { installed: true, path: "/Users/demo/.local/bin/claude", version: "2.1.220", loggedIn: false, authMethod: null, email: null, subscription: null, error: null }
            : { installed: true, path: "/Users/demo/.local/bin/claude", version: "2.1.220", loggedIn: true, authMethod: "claude.ai", email: "max@example.com", subscription: "max", error: null };
        case "runs_active":
          return Object.keys(streaming);
        case "run_cancel":
          delete streaming[a.runId];
          return null;
        case "run_start": {
          const input = a.input;
          if (!keys.anthropic) throw "Manca la chiave di Claude: aggiungila in Impostazioni → Provider";
          const convId = input.conversationId ?? uid();
          const t = Date.now();
          const run: Run = {
            id: uid(), agentId: input.agentId, conversationId: convId, title: input.text.slice(0, 58), trigger: "main", status: "running",
            startedAt: t, endedAt: null, error: null,
            steps: [{
              id: uid(), kind: "user", text: input.text || "Ecco i file allegati.", at: t,
              attachments: (input.attachments ?? []).map((p: string) => ({ name: p.split("/").pop(), size: 120_000 + Math.round(Math.random() * 900_000), kind: p.endsWith(".pdf") ? "pdf" : "text" })),
            }],
            usage: null, costEstimate: null, servedModel: null,
          };
          streaming[run.id] = true;
          setTimeout(() => {
            emit("run-started", { run });
            simulate(run, input.text);
          }, 30);
          return run;
        }
        default:
          return null;
      }
    },
    { shouldMockEvents: true },
  );
}
