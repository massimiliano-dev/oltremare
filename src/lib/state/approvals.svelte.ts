// Richieste di approvazione in attesa: una coda unica per tutte le esecuzioni.
import { api, errorText, type ApprovalRequest, type Decision } from "../api";
import { on } from "../events";
import { ui } from "./ui.svelte";

let ctx: AudioContext | null = null;
/** Due note gravi e morbide: "ho bisogno di te". */
function chime() {
  try {
    ctx ??= new AudioContext();
    const t = ctx.currentTime + 0.02;
    for (const [f, d] of [[587.3, 0], [440, 0.16]] as const) {
      const o = ctx.createOscillator();
      const g = ctx.createGain();
      o.type = "sine";
      o.frequency.value = f;
      o.connect(g).connect(ctx.destination);
      g.gain.setValueAtTime(0.0001, t + d);
      g.gain.exponentialRampToValueAtTime(0.14, t + d + 0.015);
      g.gain.exponentialRampToValueAtTime(0.0001, t + d + 0.7);
      o.start(t + d);
      o.stop(t + d + 0.75);
    }
  } catch {}
}

class ApprovalsState {
  pending = $state<ApprovalRequest[]>([]);
  now = $state(Date.now());
  current = $derived(this.pending[0] ?? null);
  private ticker: ReturnType<typeof setInterval> | null = null;

  async listen() {
    on("run-approval", ({ request }) => {
      if (this.pending.some((p) => p.id === request.id)) return;
      this.pending = [...this.pending, request];
      this.tick();
      chime();
    });
    on("run-approval-resolved", ({ approvalId, by }) => {
      const was = this.pending.find((p) => p.id === approvalId);
      this.pending = this.pending.filter((p) => p.id !== approvalId);
      if (was && by === "timeout") ui.toast(`Tempo scaduto: «${was.title}» non è stata eseguita`);
      this.tick();
    });
    this.pending = await api.approvalsPending().catch(() => []);
    this.tick();
  }

  forRun(runId: string) {
    return this.pending.filter((p) => p.runId === runId);
  }

  forStep(stepId: string) {
    return this.pending.find((p) => p.stepId === stepId) ?? null;
  }

  remaining(r: ApprovalRequest) {
    return Math.max(0, r.expiresAt - this.now);
  }

  private tick() {
    this.now = Date.now();
    if (this.pending.length && !this.ticker) this.ticker = setInterval(() => (this.now = Date.now()), 250);
    else if (!this.pending.length && this.ticker) {
      clearInterval(this.ticker);
      this.ticker = null;
    }
  }

  async answer(r: ApprovalRequest, decision: Decision, reason?: string) {
    this.pending = this.pending.filter((p) => p.id !== r.id);
    this.tick();
    try {
      await api.approvalAnswer(r.id, decision, reason);
    } catch (e) {
      ui.toast(errorText(e), "error");
    }
  }
}

export const approvals = new ApprovalsState();
