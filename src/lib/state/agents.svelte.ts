import { api, errorText, type Agent, type ToolDef } from "../api";
import { on } from "../events";
import { settings } from "./settings.svelte";
import { ui } from "./ui.svelte";
import { runs } from "./runs.svelte";

const LAST = "oltremare:agent";
/** L'agente che lavora nella vista Sviluppo */
export const DEV_AGENT = "sviluppatore";

export const AGENT_ICONS = [
  "spark", "bolt", "chat", "bulb", "folder", "mail", "calendar", "globe", "terminal", "cpu", "book", "briefcase",
  "home", "leaf", "heart", "music", "target", "shield", "search", "pen", "star", "sun", "moon", "user",
];

export const EFFORTS: { id: Agent["effort"]; label: string }[] = [
  { id: "low", label: "Basso · rapido" },
  { id: "medium", label: "Medio" },
  { id: "high", label: "Alto" },
  { id: "xhigh", label: "Molto alto" },
  { id: "max", label: "Massimo" },
];

class AgentsState {
  list = $state<Agent[]>([]);
  catalog = $state<ToolDef[]>([]);
  selectedId = $state<string | null>(null);
  selected = $derived(this.list.find((a) => a.id === this.selectedId) ?? this.list[0] ?? null);

  async load() {
    this.list = await api.agents();
    if (!this.selectedId) {
      let last: string | null = null;
      try {
        last = localStorage.getItem(LAST);
      } catch {}
      this.selectedId = this.list.some((a) => a.id === last) ? last : (this.list.find((a) => a.id === "assistente")?.id ?? this.list[0]?.id ?? null);
    }
  }

  async init() {
    api.toolCatalog().then((c) => (this.catalog = c)).catch(() => {});
    await this.load();
    on("agents-changed", () => this.load());
  }

  byId(id: string | null | undefined) {
    return this.list.find((a) => a.id === id) ?? null;
  }

  select(id: string) {
    // Tutti gli agenti stanno nella chat: lo sviluppo vero si fa nello Studio
    ui.view = "chat";
    if (id === this.selected?.id) return;
    this.selectedId = id;
    try {
      localStorage.setItem(LAST, id);
    } catch {}
    runs.selectAgent(id);
  }

  blank(): Agent {
    const s = settings.s;
    const provider: Agent["provider"] = settings.state?.keys.anthropic
      ? "anthropic"
      : settings.claudeCode?.loggedIn
        ? "claude-code"
        : settings.state?.keys.openai
          ? "openai"
          : "anthropic";
    return {
      id: "",
      name: "",
      icon: "spark",
      hue: 275,
      description: "",
      systemPrompt: "",
      provider,
      model:
        provider === "anthropic"
          ? (s?.anthropicModel ?? "claude-opus-5-5")
          : provider === "claude-code"
            ? (s?.claudeCodeModel ?? "default")
            : (s?.openaiModel ?? "gpt-5"),
      effort: "medium",
      maxTokens: 32000,
      maxTurns: 40,
      tools: ["fs", "apps", "clipboard"],
      policy: { readDefault: "allow", writeDefault: "ask", dangerDefault: "ask", perTool: {}, approvalTimeoutS: 180, allowSession: true },
      allowedPaths: [],
      mailAccounts: [],
      browserDomains: [],
      voiceReply: "whenAsked",
      triggers: [],
      system: false,
      createdAt: 0,
      updatedAt: 0,
    };
  }

  async save(a: Agent): Promise<Agent | null> {
    try {
      const saved = await api.saveAgent(a);
      await this.load();
      return saved;
    } catch (e) {
      ui.toast(errorText(e), "error");
      return null;
    }
  }

  /** Passa l'agente all'abbonamento Claude (Claude Code), con il modello predefinito del piano. */
  async useSubscription(a: Agent, quiet = false) {
    const saved = await this.save({ ...$state.snapshot(a), provider: "claude-code", model: settings.s?.claudeCodeModel ?? "default", effort: a.effort || "medium" } as Agent);
    if (saved && !quiet) ui.toast(`«${saved.name}» ora usa il tuo abbonamento Claude`);
  }

  /** Gli agenti di sistema creati all'inizio puntano a Claude con chiave API: se la chiave non c'è
   *  ma l'abbonamento è collegato, li si sposta sull'abbonamento. */
  async adoptSubscription() {
    if (settings.state?.keys.anthropic || !settings.claudeCode?.loggedIn) return;
    for (const a of this.list) {
      if (a.system && a.provider === "anthropic") await this.useSubscription(a, true);
    }
  }

  async remove(a: Agent) {
    try {
      await api.deleteAgent(a.id);
      await this.load();
      if (this.selectedId === a.id) this.select(this.list[0]?.id ?? "");
      ui.toast(`«${a.name}» eliminato`);
    } catch (e) {
      ui.toast(errorText(e), "error");
    }
  }

  async duplicate(a: Agent) {
    try {
      const copy = await api.duplicateAgent(a.id);
      await this.load();
      this.select(copy.id);
    } catch (e) {
      ui.toast(errorText(e), "error");
    }
  }
}

export const agents = new AgentsState();
