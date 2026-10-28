import { api, errorText, type ClaudeCodeStatus, type Provider, type Settings, type SettingsState } from "../api";
import { on } from "../events";
import { theme } from "./theme.svelte";
import { ui } from "./ui.svelte";
import { local } from "./local.svelte";

export const PROVIDERS: { id: Provider; label: string; sub: string }[] = [
  { id: "anthropic", label: "Claude", sub: "chiave API · a consumo" },
  { id: "claude-code", label: "Abbonamento", sub: "Claude Pro o Max" },
  { id: "openai", label: "GPT", sub: "OpenAI o compatibile" },
  { id: "local", label: "Locale", sub: "sul tuo computer, gratis" },
];

export const FALLBACK_MODELS: Record<Provider, string[]> = {
  anthropic: ["claude-opus-5-5", "claude-sonnet-5-5", "claude-haiku-5-5", "claude-fable-5-1"],
  "claude-code": ["default", "opus", "sonnet", "haiku"],
  openai: ["gpt-5", "gpt-5-mini"],
  local: [],
};

export function providerLabel(p: Provider | undefined) {
  if (p === "claude-code") return "Abbonamento";
  return PROVIDERS.find((x) => x.id === p)?.label ?? "Claude";
}

const CC_MODELS: Record<string, string> = { default: "quello del piano", opus: "Opus", sonnet: "Sonnet", haiku: "Haiku" };

/** Nome breve del modello per liste ed etichette. */
export function modelLabel(p: Provider, model: string) {
  if (p === "claude-code") return CC_MODELS[model] ?? model.replace(/^claude-/, "");
  if (p === "local") return model ? local.label(model) : "modello predefinito";
  return model.replace(/^claude-/, "");
}

class SettingsStore {
  state = $state<SettingsState | null>(null);
  s = $derived(this.state?.settings ?? null);
  /** Claude Code installato e collegato al tuo account (null = non ancora controllato) */
  claudeCode = $state<ClaudeCodeStatus | null>(null);
  checkingClaudeCode = $state(false);

  async load() {
    this.state = await api.settings();
    this.applyLook(this.state.settings);
    on("settings-changed", (p) => {
      this.state = p;
      this.applyLook(p.settings);
    });
  }

  private applyLook(s: Settings) {
    theme.set(s.theme);
    theme.look({ accentHue: s.accentHue, accentChroma: s.accentChroma, density: s.density, fontScale: s.fontScale });
  }

  async checkClaudeCode() {
    this.checkingClaudeCode = true;
    try {
      this.claudeCode = await api.claudeCodeStatus();
    } catch (e) {
      this.claudeCode = { installed: false, path: null, version: null, loggedIn: false, authMethod: null, email: null, subscription: null, error: errorText(e) };
    } finally {
      this.checkingClaudeCode = false;
    }
  }

  async save(patch: Partial<Settings>) {
    if (!this.state) return;
    const next = { ...this.state.settings, ...patch };
    this.state.settings = next;
    this.applyLook(next);
    try {
      await api.saveSettings(next);
    } catch (e) {
      ui.toast(errorText(e), "error");
    }
  }

  ready(p: Provider): boolean {
    if (p === "local") return local.installed.length > 0;
    // Finché non si è controllato, si dà fiducia: l'errore eventuale arriva con la prima risposta
    if (p === "claude-code") return this.claudeCode?.loggedIn ?? true;
    return this.state?.keys[p] ?? false;
  }
}

export const settings = new SettingsStore();
