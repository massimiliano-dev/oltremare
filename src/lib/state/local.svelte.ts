// AI locale: stato del motore llama.cpp, libreria dei modelli, download.
import { api, errorText, type DownloadProgress, type Hardware, type ModelEntry, type SidecarStatus } from "../api";
import { on } from "../events";
import { ui } from "./ui.svelte";

class LocalState {
  status = $state<SidecarStatus | null>(null);
  catalog = $state<ModelEntry[]>([]);
  hw = $state<Hardware | null>(null);
  downloads = $state<Record<string, DownloadProgress>>({});
  log = $state<string[]>([]);

  llms = $derived(this.catalog.filter((m) => m.kind === "llm"));
  installed = $derived(this.llms.filter((m) => m.installed));
  active = $derived(this.status && this.status.state !== "stopped" ? this.status : null);

  async init() {
    on("sidecar-status", (s) => (this.status = s));
    on("model-download", (p) => {
      this.downloads[p.id] = p;
      if (p.done) {
        if (p.error) ui.toast(p.error, "error");
        else if (p.id !== "motore") {
          const m = this.catalog.find((x) => x.id === p.id);
          if (m) ui.toast(`${m.label} è pronto`);
        }
        setTimeout(() => delete this.downloads[p.id], 1500);
        void this.refresh();
      }
    });
    await this.refresh();
  }

  async refresh() {
    const [status, catalog, hw] = await Promise.all([api.llamaStatus(), api.modelsCatalog(), api.hwInfo()]).catch(() => [null, [], null] as const);
    this.status = status;
    this.catalog = [...catalog];
    this.hw = hw;
  }

  label(id: string | null | undefined) {
    if (!id) return "";
    return this.catalog.find((m) => m.id === id)?.label ?? id.replace(/^custom:/, "").replace(/\.gguf$/, "");
  }

  /** Quanto sta comodo in memoria: "ok" | "tight" | "no" */
  fit(m: ModelEntry): "ok" | "tight" | "no" {
    if (!this.hw || !m.ramNeededGb) return "ok";
    const gb = this.hw.ramTotal / 1e9;
    if (m.ramNeededGb <= gb * 0.7) return "ok";
    if (m.ramNeededGb <= gb * 0.95) return "tight";
    return "no";
  }

  async download(id: string) {
    this.downloads[id] = { id, received: 0, total: this.catalog.find((m) => m.id === id)?.sizeBytes ?? 0, bytesPerSec: 0, done: false, error: null, phase: "download" };
    try {
      await api.modelDownload(id);
    } catch (e) {
      delete this.downloads[id];
      ui.toast(errorText(e), "error");
    }
  }

  async cancel(id: string) {
    await api.modelDownloadCancel(id).catch(() => {});
    delete this.downloads[id];
  }

  async remove(m: ModelEntry) {
    try {
      await api.modelDelete(m.id);
      await this.refresh();
      ui.toast(`${m.label} eliminato`);
    } catch (e) {
      ui.toast(errorText(e), "error");
    }
  }

  async start(id: string) {
    try {
      await api.llamaStart(id);
    } catch (e) {
      ui.toast(errorText(e), "error");
    }
  }

  async stop() {
    await api.llamaStop().catch(() => {});
  }

  async importFile() {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const r = await open({ multiple: false, title: "Importa un modello GGUF", filters: [{ name: "Modelli GGUF", extensions: ["gguf"] }] });
      if (!r || Array.isArray(r)) return;
      await api.modelImport(r);
      await this.refresh();
      ui.toast("Modello importato");
    } catch (e) {
      ui.toast(errorText(e), "error");
    }
  }

  async loadLog() {
    this.log = await api.llamaLog().catch(() => []);
  }
}

export const local = new LocalState();
