import type { Agent } from "../api";

export interface Toast {
  id: number;
  text: string;
  kind: "info" | "error";
  action?: { label: string; run: () => void };
}

export type SettingsTab = "general" | "appearance" | "providers" | "local" | "voice" | "assistant" | "notifications" | "accounts" | "tools" | "shortcuts";

class UiState {
  readonly isMac = navigator.userAgent.includes("Mac");
  showSettings = $state(false);
  settingsTab = $state<SettingsTab>("general");
  showPalette = $state(false);
  /** Agente in modifica: una copia, oppure "new" per crearne uno */
  editing = $state<Agent | "new" | null>(null);
  toasts = $state<Toast[]>([]);
  private toastId = 0;

  toast(text: string, kind: Toast["kind"] = "info", action?: Toast["action"], ms = 4500) {
    const id = ++this.toastId;
    this.toasts = [...this.toasts.slice(-2), { id, text, kind, action }];
    setTimeout(() => this.dismiss(id), kind === "error" ? ms + 2500 : ms);
  }

  dismiss(id: number) {
    this.toasts = this.toasts.filter((t) => t.id !== id);
  }

  openSettings(tab: SettingsTab = "general") {
    this.settingsTab = tab;
    this.showSettings = true;
  }
}

export const ui = new UiState();
