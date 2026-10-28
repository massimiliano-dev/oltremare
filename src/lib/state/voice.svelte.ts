// Voce: stato dell'ascolto, ultima frase capita, comandi.
import { api, errorText, type VoiceStatus } from "../api";
import { on } from "../events";
import { ui } from "./ui.svelte";
import { agents } from "./agents.svelte";
import { runs } from "./runs.svelte";

class VoiceState {
  status = $state<VoiceStatus | null>(null);
  recording = $state(false);
  transcript = $state("");
  private clearTimer: ReturnType<typeof setTimeout> | null = null;

  ready = $derived(!!this.status?.models.vad && !!this.status?.models.stt);

  async init() {
    on("voice-status", (s) => {
      this.recording = s.recording;
      if (this.status) this.status.wakeListening = s.listening;
      if (s.error) ui.toast(s.error, "error");
    });
    on("voice-transcript", (t) => {
      this.transcript = t.empty ? "Non ho sentito nulla" : t.text;
      if (this.clearTimer) clearTimeout(this.clearTimer);
      this.clearTimer = setTimeout(() => (this.transcript = ""), 4000);
    });
    on("voice-error", (e) => ui.toast(e, "error", e.includes("Impostazioni") ? { label: "Apri", run: () => ui.openSettings("voice") } : undefined));
    on("voice-run", async ({ conversationId, agentId, runId }) => {
      // La richiesta a voce si vede nella chat
      await runs.openRun(agentId, conversationId);
      runs.selectedRunId = runId;
    });
    await this.refresh();
  }

  async refresh() {
    this.status = await api.voiceStatus().catch(() => null);
  }

  /** Pulsante del microfono: ascolta un comando, oppure annulla se sta già ascoltando. */
  async toggle() {
    if (!this.ready) {
      ui.toast("Per parlare servono i modelli vocali", "info", { label: "Scarica", run: () => ui.openSettings("voice") });
      return;
    }
    try {
      if (this.recording) await api.voiceCancel();
      else await api.voiceCommand();
    } catch (e) {
      ui.toast(errorText(e), "error");
    }
  }
}

export const voice = new VoiceState();
