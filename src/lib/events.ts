// Eventi dal backend, tipizzati per nome.
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { McpStatus, Notifica, DownloadProgress, SidecarStatus, ApprovalRequest, AvatarState, Decision, Run, RunStatus, SettingsState, Step, Usage } from "./api";

export interface EventMap {
  "run-started": { run: Run };
  "run-delta": { runId: string; stepId: string; kind: "text" | "thinking"; delta: string };
  "run-step": { runId: string; step: Step };
  "run-done": {
    runId: string;
    conversationId: string;
    status: RunStatus;
    usage: Usage;
    costEstimate: number | null;
    servedModel: string | null;
    error: string | null;
    endedAt: number;
  };
  "run-status": { runId: string; status: RunStatus };
  "run-approval": { runId: string; request: ApprovalRequest };
  "run-approval-resolved": { approvalId: string; runId: string; decision: Decision; by: "user" | "timeout" | "cancel" };
  "run-progress": { runId: string; text: string };
  "sidecar-status": SidecarStatus;
  "model-download": DownloadProgress;
  "voice-status": { listening: boolean; recording: boolean; error: string | null };
  "voice-transcript": { text: string; final: boolean; empty?: boolean };
  "voice-wakeword": null;
  "voice-run": { runId: string; conversationId: string; agentId: string };
  "voice-error": string;
  "accounts-changed": null;
  "oauth-url": string;
  "avatar-state": { state: AvatarState; level: number };
  "settings-changed": SettingsState;
  "agents-changed": null;
  "prefs-changed": { key: string };
  "notification-new": Notifica;
  "notification-alert": { notification: Notifica; rule: string };
  "briefing-started": { runId: string; conversationId: string };
  "trigger-fired": { agentId: string; kind: string; runId: string };
  "open-run": { runId: string; conversationId: string; agentId: string };
  "open-settings": string;
  "mcp-status": McpStatus[];
  "palette-open": { agentId: string | null };
}

export function on<K extends keyof EventMap>(name: K, cb: (payload: EventMap[K]) => void): Promise<UnlistenFn> {
  return listen<EventMap[K]>(name, (e) => cb(e.payload));
}
