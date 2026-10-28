// Esecuzioni e conversazioni. I pezzi di testo in streaming si accumulano e si applicano
// una volta per fotogramma: decine di aggiornamenti al secondo non rallentano l'interfaccia.
import { api, errorText, type Conversation, type Run, type Step } from "../api";
import { on } from "../events";
import { agents } from "./agents.svelte";
import { avatar } from "./avatar.svelte";
import { ui } from "./ui.svelte";

const ACTIVE = new Set(["running", "awaitingApproval"]);

class RunsState {
  byId = $state<Record<string, Run>>({});
  conversations = $state<Conversation[]>([]);
  conversationId = $state<string | null>(null);
  /** Esecuzioni della conversazione aperta, in ordine */
  order = $state<string[]>([]);
  selectedRunId = $state<string | null>(null);
  loading = $state(false);
  /** Avvisi di avanzamento per esecuzione (es. "Avvio l'AI locale…") */
  progress = $state<Record<string, string>>({});
  sending = $state(false);

  current = $derived(this.order.map((id) => this.byId[id]).filter(Boolean));
  active = $derived(this.current.find((r) => ACTIVE.has(r.status)) ?? null);
  selected = $derived((this.selectedRunId && this.byId[this.selectedRunId]) || this.current.at(-1) || null);
  conversation = $derived(this.conversations.find((c) => c.id === this.conversationId) ?? null);

  private pending = new Map<string, { runId: string; kind: "text" | "thinking"; text: string }>();
  private frame = 0;
  /** Durante l'invio di una conversazione nuova, la prima esecuzione che arriva è nostra */
  private expecting = false;

  listen() {
    on("run-started", ({ run }) => {
      this.byId[run.id] ??= run;
      // Solo esecuzioni dell'agente selezionato: un altro agente partito da solo non entra in questa chat
      const mine = run.agentId === agents.selected?.id && (run.conversationId === this.conversationId || (this.expecting && this.conversationId === null));
      if (mine) this.adopt(run);
    });
    on("run-progress", (p) => (this.progress[p.runId] = p.text));
    on("run-delta", (p) => {
      if (this.progress[p.runId]) delete this.progress[p.runId];
      const key = p.stepId;
      const cur = this.pending.get(key);
      if (cur) cur.text += p.delta;
      else this.pending.set(key, { runId: p.runId, kind: p.kind, text: p.delta });
      if (p.kind === "text") avatar.pulse();
      this.frame ||= requestAnimationFrame(() => this.flush());
    });
    on("run-step", ({ runId, step }) => {
      this.flush();
      const run = this.byId[runId];
      if (!run) return;
      const i = run.steps.findIndex((s) => s.id === step.id);
      if (i >= 0) run.steps[i] = step;
      else run.steps.push(step);
    });
    on("run-status", ({ runId, status }) => {
      const run = this.byId[runId];
      if (run && ACTIVE.has(run.status)) run.status = status;
    });
    on("run-done", (p) => {
      this.flush();
      delete this.progress[p.runId];
      const run = this.byId[p.runId];
      if (run) {
        run.status = p.status;
        run.usage = p.usage;
        run.costEstimate = p.costEstimate;
        run.servedModel = p.servedModel;
        run.error = p.error;
        run.endedAt = p.endedAt;
      }
      if (p.status === "error" && p.error && p.conversationId !== this.conversationId) ui.toast(p.error, "error");
      // La conversazione sale in cima alla lista
      const c = this.conversations.find((x) => x.id === p.conversationId);
      if (c) {
        c.updatedAt = p.endedAt;
        this.conversations = [c, ...this.conversations.filter((x) => x.id !== c.id)];
      }
    });
  }

  private adopt(run: Run) {
    if (this.conversationId === null) this.conversationId = run.conversationId;
    if (!this.order.includes(run.id)) this.order = [...this.order, run.id];
    this.selectedRunId = run.id;
  }

  private flush() {
    if (this.frame) cancelAnimationFrame(this.frame);
    this.frame = 0;
    for (const [stepId, p] of this.pending) {
      const run = this.byId[p.runId];
      if (!run) continue;
      let step = run.steps.find((s) => s.id === stepId);
      if (!step) {
        run.steps.push({ id: stepId, kind: p.kind, text: "", at: Date.now() } satisfies Step);
        step = run.steps[run.steps.length - 1];
      }
      step.text += p.text;
    }
    this.pending.clear();
  }

  async selectAgent(agentId: string) {
    this.newConversation();
    this.conversations = await api.conversations(agentId).catch(() => []);
  }

  newConversation() {
    this.conversationId = null;
    this.order = [];
    this.selectedRunId = null;
  }

  /** Apre un'esecuzione partita altrove (briefing, trigger, voce, palette). */
  async openRun(agentId: string, conversationId: string) {
    if (agents.selected?.id !== agentId) {
      agents.selectedId = agentId;
      try {
        localStorage.setItem("oltremare:agent", agentId);
      } catch {}
      this.conversations = await api.conversations(agentId).catch(() => []);
    }
    this.conversationId = null;
    await this.openConversation(conversationId);
  }

  async openConversation(id: string) {
    if (id === this.conversationId) return;
    this.loading = true;
    this.conversationId = id;
    try {
      const list = await api.conversationRuns(id);
      if (this.conversationId !== id) return;
      // La conversazione si apre sempre sotto il suo agente
      const owner = list[0]?.agentId;
      if (owner && owner !== agents.selected?.id && agents.byId(owner)) {
        agents.selectedId = owner;
        try {
          localStorage.setItem("oltremare:agent", owner);
        } catch {}
        this.conversations = await api.conversations(owner).catch(() => []);
        if (this.conversationId !== id) return;
      }
      for (const r of list) {
        // Un'esecuzione in corso tiene i passi già arrivati in diretta
        if (!this.byId[r.id] || !ACTIVE.has(this.byId[r.id].status)) this.byId[r.id] = r;
      }
      this.order = list.map((r) => r.id);
      this.selectedRunId = list.at(-1)?.id ?? null;
    } catch (e) {
      ui.toast(errorText(e), "error");
    } finally {
      this.loading = false;
    }
  }

  async send(text: string, attachments: string[] = []): Promise<boolean> {
    const agent = agents.selected;
    if (!agent || (!text.trim() && !attachments.length) || this.sending) return false;
    this.sending = true;
    // Una conversazione aperta di un altro agente non si continua: se ne inizia una nuova
    const current = this.order.map((id) => this.byId[id]).find(Boolean);
    if (this.conversationId && current && current.agentId !== agent.id) this.newConversation();
    const isNew = this.conversationId === null;
    this.expecting = isNew;
    try {
      const run = await api.runStart({ agentId: agent.id, conversationId: this.conversationId, text, source: "main", attachments });
      this.byId[run.id] ??= run;
      this.adopt(this.byId[run.id]);
      if (isNew) {
        const now = Date.now();
        this.conversations = [{ id: run.conversationId, agentId: agent.id, title: run.title, createdAt: now, updatedAt: now }, ...this.conversations];
      }
      return true;
    } catch (e) {
      ui.toast(errorText(e), "error", errorText(e).includes("Impostazioni") ? { label: "Apri", run: () => ui.openSettings("providers") } : undefined);
      return false;
    } finally {
      this.expecting = false;
      this.sending = false;
    }
  }

  cancel(runId: string) {
    api.runCancel(runId).catch(() => {});
  }

  async deleteConversation(c: Conversation) {
    try {
      await api.deleteConversation(c.id);
      this.conversations = this.conversations.filter((x) => x.id !== c.id);
      if (this.conversationId === c.id) this.newConversation();
    } catch (e) {
      ui.toast(errorText(e), "error");
    }
  }
}

export const runs = new RunsState();
