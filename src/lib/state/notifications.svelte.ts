import { api, errorText, type BriefingInfo, type Notifica, type NotificationSources } from "../api";
import { on } from "../events";
import { ui } from "./ui.svelte";

export const SOURCE_LABEL: Record<Notifica["source"], string> = { system: "Sistema", teams: "Teams", slack: "Slack", mail: "Posta" };

async function systemNotify(title: string, body: string) {
  try {
    const n = await import("@tauri-apps/plugin-notification");
    if (!(await n.isPermissionGranted()) && (await n.requestPermission()) !== "granted") return;
    n.sendNotification({ title, body });
  } catch {}
}

class NotificationsStore {
  list = $state<Notifica[]>([]);
  sources = $state<NotificationSources | null>(null);
  briefing = $state<BriefingInfo | null>(null);
  loading = $state(false);
  briefingBusy = $state(false);
  unread = $derived(this.list.filter((n) => !n.read).length);
  /** Raggruppate per app, le più recenti prima */
  groups = $derived.by(() => {
    const m = new Map<string, Notifica[]>();
    for (const n of this.list) m.set(n.app, [...(m.get(n.app) ?? []), n]);
    return [...m.entries()].sort((a, b) => b[1][0].at - a[1][0].at);
  });

  async init() {
    on("notification-new", (n) => {
      if (!this.list.some((x) => x.id === n.id)) this.list = [n, ...this.list].slice(0, 300);
    });
    on("notification-alert", ({ notification: n }) => {
      ui.toast(`${n.app}: ${n.title}${n.body ? " — " + n.body.slice(0, 120) : ""}`, "info", undefined, 8000);
      if (!document.hasFocus()) systemNotify(`${n.app} · ${n.title}`, n.body.slice(0, 200));
    });
    on("briefing-started", (b) => {
      this.briefing = { date: new Date().toISOString().slice(0, 10), ...b };
    });
    await Promise.all([this.refresh(), this.loadBriefing()]);
  }

  async refresh() {
    this.loading = true;
    try {
      this.list = await api.notifications(48);
    } catch {
      this.list = [];
    } finally {
      this.loading = false;
    }
  }

  async loadSources() {
    this.sources = await api.notificationSources().catch(() => null);
  }

  async loadBriefing() {
    this.briefing = await api.briefingLatest().catch(() => null);
  }

  async markRead() {
    await api.notificationsMarkRead().catch(() => {});
    this.list = this.list.map((n) => ({ ...n, read: true }));
  }

  /** Avvia il briefing adesso; restituisce l'esecuzione da aprire. */
  async briefingNow() {
    if (this.briefingBusy) return null;
    this.briefingBusy = true;
    try {
      const run = await api.briefingNow();
      this.briefing = { date: new Date().toISOString().slice(0, 10), runId: run.id, conversationId: run.conversationId };
      return run;
    } catch (e) {
      ui.toast(errorText(e), "error");
      return null;
    } finally {
      this.briefingBusy = false;
    }
  }
}

export const notifications = new NotificationsStore();
