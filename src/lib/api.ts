// Contratto con il backend Rust: comandi `invoke` tipizzati e tipi condivisi (camelCase).
import { invoke } from "@tauri-apps/api/core";

export type Provider = "anthropic" | "claude-code" | "openai" | "local";
export type Policy = "allow" | "ask" | "deny";
export type Effort = "low" | "medium" | "high" | "xhigh" | "max";

export interface AgentPolicy {
  readDefault: Policy;
  writeDefault: Policy;
  dangerDefault: Policy;
  perTool: Record<string, Policy>;
  approvalTimeoutS: number;
  allowSession: boolean;
}

export interface Agent {
  id: string;
  name: string;
  icon: string;
  hue: number;
  description: string;
  systemPrompt: string;
  provider: Provider;
  model: string;
  effort: Effort | "";
  maxTokens: number;
  maxTurns: number;
  tools: string[];
  policy: AgentPolicy;
  allowedPaths: string[];
  mailAccounts: string[];
  browserDomains: string[];
  voiceReply: "always" | "whenAsked" | "never";
  triggers: Trigger[];
  system: boolean;
  createdAt: number;
  updatedAt: number;
}

export type StepKind = "user" | "text" | "thinking" | "tool" | "notice";

export interface Step {
  id: string;
  kind: StepKind;
  text: string;
  at: number;
  endedAt?: number;
  viaVoice?: boolean;
  attachments?: { name: string; size: number; kind: "pdf" | "image" | "text" }[];
  tool?: string;
  category?: string;
  title?: string;
  input?: unknown;
  status?: string;
  result?: unknown;
  error?: string;
}

export type RunStatus = "running" | "awaitingApproval" | "done" | "error" | "cancelled" | "refused";

export interface Usage {
  inputTokens: number;
  outputTokens: number;
  cacheRead: number;
  cacheWrite: number;
}

export interface Run {
  id: string;
  agentId: string;
  conversationId: string;
  title: string;
  trigger: string;
  status: RunStatus;
  startedAt: number;
  endedAt: number | null;
  error: string | null;
  steps: Step[];
  usage: Usage | null;
  costEstimate: number | null;
  servedModel: string | null;
}

export interface Conversation {
  id: string;
  agentId: string;
  title: string;
  createdAt: number;
  updatedAt: number;
}

export interface Settings {
  userName: string;
  language: string;
  anthropicModel: string;
  openaiModel: string;
  openaiBaseUrl: string;
  claudeCodePath: string;
  claudeCodeModel: string;
  localModel: string;
  theme: "system" | "light" | "dark";
  avatarFace: "character" | "orb";
  sendWithEnter: boolean;
  showThinking: boolean;
  voiceListen: boolean;
  voiceStt: string;
  voiceTts: string;
  voiceSpeed: number;
  voiceDevice: string;
  voiceAgent: string;
  voiceHotkey: string;
  companion: boolean;
  msClientId: string;
  googleClientId: string;
  googleClientSecret: string;
  notificationsSystem: boolean;
  notificationsTeams: boolean;
  notificationsSlack: boolean;
  notificationsMail: boolean;
  notificationsLocalOnly: boolean;
  notificationRules: NotificationRule[];
  quietFrom: string;
  quietTo: string;
  briefingEnabled: boolean;
  briefingTime: string;
  briefingDays: number[];
  accentHue: number;
  accentChroma: number;
  density: "comfortable" | "compact";
  fontScale: number;
  paletteHotkey: string;
  autostart: boolean;
  layout: string;
  mcpServers: McpServer[];
  editorApp: string;
  devMode: "editor" | "oltremare";
  companionStyle: "island" | "floating";
  browserVisible: boolean;
  computerWaitIdle: boolean;
}

export interface DevEntry {
  name: string;
  path: string;
  dir: boolean;
}

export interface DevFile {
  path: string;
  content: string;
  binary: boolean;
  truncated: boolean;
  size: number;
  ext: string;
}

export interface DevGit {
  repo: boolean;
  branch?: string;
  last?: string;
  changes?: { status: string; path: string; rel: string }[];
}

export interface DevItem {
  kind: "user" | "text" | "tool";
  text?: string;
  tool?: string;
  title?: string;
  input?: unknown;
  result?: string;
  error?: boolean;
  done?: boolean;
  at: string;
}

export interface DevSession {
  id: string | null;
  items: DevItem[];
  sessions: { id: string; title: string; updatedAt: number }[];
  running: boolean;
  here?: boolean;
  stuck?: boolean;
}

export interface DevProcess {
  id: string;
  name: string;
  command: string;
  folder: string;
  running: boolean;
  exitCode: number | null;
  urls: string[];
  log: string;
}

export interface McpServer {
  id: string;
  name: string;
  kind: "http" | "stdio";
  url: string;
  headersHelper: string;
  command: string;
  args: string[];
  enabled: boolean;
}

export interface McpStatus {
  id: string;
  name: string;
  connected: boolean;
  connecting: boolean;
  tools: string[];
  error: string | null;
}

export interface NotificationRule {
  id: string;
  app: string;
  contains: string;
  action: "alert" | "speak" | "agent";
  agentId: string;
  enabled: boolean;
}

export interface Notifica {
  id: string;
  at: number;
  app: string;
  appId: string;
  title: string;
  body: string;
  source: "system" | "teams" | "slack" | "mail";
  read: boolean;
}

export interface NotificationSources {
  systemSupported: boolean;
  fullDisk: boolean;
  teamsAccounts: string[];
  slack: string | null;
}

export interface BriefingInfo {
  date: string;
  runId?: string;
  conversationId?: string;
  error?: string;
}

export type Trigger =
  | { kind: "voice"; phrases: string[] }
  | { kind: "hotkey"; accelerator: string; input: "palette" | "clipboard" | "none"; prompt: string }
  | { kind: "cron"; expr: string; prompt: string; catchUp: boolean }
  | { kind: "folder"; path: string; events: ("created" | "modified" | "deleted")[]; glob: string; recursive: boolean; prompt: string }
  | { kind: "mail"; accountId: string; from: string; subjectContains: string; prompt: string }
  | { kind: "notification"; app: string; contains: string; prompt: string };

export interface VoiceStatus {
  wakeListening: boolean;
  active: boolean;
  speaking: boolean;
  models: { vad: boolean; stt: boolean; tts: boolean };
}

export interface SettingsState {
  settings: Settings;
  keys: { anthropic: boolean; openai: boolean };
}

export interface ProviderTest {
  ok: boolean;
  ms: number;
  models: number;
  error: string | null;
}

export interface ClaudeCodeStatus {
  installed: boolean;
  path: string | null;
  version: string | null;
  loggedIn: boolean;
  authMethod: string | null;
  email: string | null;
  subscription: string | null;
  error: string | null;
}

export type Risk = "read" | "write" | "danger";
export type ToolCategory = "fs" | "shell" | "apps" | "clipboard" | "computer" | "browser" | "mail" | "calendar" | "tasks" | "notifications" | "dev" | "media" | "web" | "other" | `mcp:${string}`;

export interface ToolDef {
  name: string;
  label: string;
  description: string;
  category: ToolCategory;
  risk: Risk;
  defaultPolicy: Policy;
}

export interface ApprovalRequest {
  id: string;
  runId: string;
  stepId: string;
  agentId: string;
  agentName: string;
  tool: string;
  category: ToolCategory;
  risk: Risk;
  title: string;
  summary: string;
  input: Record<string, unknown>;
  diff: string | null;
  expiresAt: number;
  allowSession: boolean;
}

export type Decision = "allow" | "allowSession" | "allowAlways" | "deny";

export interface AuditEntry {
  at: number;
  runId: string;
  agentId: string;
  tool: string;
  input: unknown;
  decision: string;
  by: string;
  ok: boolean;
  error: string | null;
}

export interface SidecarStatus {
  state: "stopped" | "installing" | "starting" | "loading" | "ready" | "error";
  port: number | null;
  build: string;
  backend: "metal" | "cuda" | "vulkan" | "cpu";
  model: string | null;
  message: string | null;
  installed: boolean;
}

export interface ModelEntry {
  id: string;
  kind: "llm" | "stt" | "tts" | "vad";
  label: string;
  note: string;
  repo: string;
  sizeBytes: number;
  ramNeededGb: number;
  ctx: number;
  supportsTools: boolean;
  supportsVision: boolean;
  recommended: boolean;
  installed: boolean;
  custom: boolean;
}

export interface Hardware {
  ramTotal: number;
  ramAvailable: number;
  cpu: string;
  backend: string;
  os: string;
}

export interface DownloadProgress {
  id: string;
  received: number;
  total: number;
  bytesPerSec: number;
  done: boolean;
  error: string | null;
  phase: "download" | "done" | "error";
}

export interface Permissions {
  accessibility: boolean;
  screen: boolean;
  fullDisk: boolean;
  needed: boolean;
}

export interface Account {
  id: string;
  provider: "microsoft" | "google";
  email: string;
  name: string;
  createdAt: number;
  teams: boolean;
}

export type AvatarState = "sleep" | "idle" | "listening" | "thinking" | "speaking" | "working" | "asking" | "alert" | "error";

export interface RunStartInput {
  agentId: string;
  conversationId: string | null;
  text: string;
  source?: "main" | "palette" | "voice" | "trigger";
  attachments?: string[];
}

export const api = {
  settings: () => invoke<SettingsState>("settings_get"),
  saveSettings: (settings: Settings) => invoke<void>("settings_save", { settings }),
  setKey: (provider: Provider, key: string) => invoke<void>("provider_key_set", { provider, key }),
  models: (provider: Provider) => invoke<string[]>("provider_models", { provider }),
  testProvider: (provider: Provider) => invoke<ProviderTest>("provider_test", { provider }),

  prefsGet: <T>(key: string) => invoke<T | null>("prefs_get", { key }),
  prefsSet: (key: string, value: unknown) => invoke<void>("prefs_set", { key, value }),

  agents: () => invoke<Agent[]>("agents_list"),
  saveAgent: (agent: Agent) => invoke<Agent>("agent_save", { agent }),
  deleteAgent: (id: string) => invoke<void>("agent_delete", { id }),
  duplicateAgent: (id: string) => invoke<Agent>("agent_duplicate", { id }),

  runStart: (input: RunStartInput) => invoke<Run>("run_start", { input }),
  runCancel: (runId: string) => invoke<void>("run_cancel", { runId }),
  runsCancelAll: () => invoke<void>("runs_cancel_all"),
  runsActive: () => invoke<string[]>("runs_active"),
  run: (runId: string) => invoke<Run>("run_get", { runId }),
  runs: (agentId: string | null, limit = 50, offset = 0) => invoke<Run[]>("runs_list", { agentId, limit, offset }),
  conversations: (agentId: string) => invoke<Conversation[]>("conversations_list", { agentId }),
  conversationRuns: (conversationId: string) => invoke<Run[]>("conversation_runs", { conversationId }),
  deleteConversation: (conversationId: string) => invoke<void>("conversation_delete", { conversationId }),

  avatarState: () => invoke<AvatarState>("avatar_state"),
  claudeCodeStatus: () => invoke<ClaudeCodeStatus>("claude_code_status"),
  attachmentTypes: () => invoke<string[]>("attachment_types"),
  toolCatalog: () => invoke<ToolDef[]>("tool_catalog"),
  approvalAnswer: (approvalId: string, decision: Decision, reason?: string) => invoke<void>("approval_answer", { approvalId, decision, reason }),
  approvalsPending: () => invoke<ApprovalRequest[]>("approvals_pending"),
  stopEverything: () => invoke<void>("stop_everything"),
  audit: (limit = 100, offset = 0) => invoke<AuditEntry[]>("audit_list", { limit, offset }),
  llamaStatus: () => invoke<SidecarStatus>("llama_status"),
  llamaStart: (modelId: string) => invoke<void>("llama_start", { modelId }),
  llamaStop: () => invoke<void>("llama_stop"),
  llamaLog: () => invoke<string[]>("llama_log"),
  modelsCatalog: (kind?: ModelEntry["kind"]) => invoke<ModelEntry[]>("models_catalog", { kind }),
  modelDownload: (id: string) => invoke<void>("model_download", { id }),
  modelDownloadCancel: (id: string) => invoke<void>("model_download_cancel", { id }),
  modelDelete: (id: string) => invoke<void>("model_delete", { id }),
  modelImport: (path: string) => invoke<string>("model_import", { path }),
  hwInfo: () => invoke<Hardware>("hw_info"),
  modelsFolder: () => invoke<string>("models_folder"),
  voiceStatus: () => invoke<VoiceStatus>("voice_status"),
  voiceDevices: () => invoke<string[]>("voice_devices"),
  voiceListenSet: (enabled: boolean) => invoke<void>("voice_listen_set", { enabled }),
  voiceCommand: () => invoke<void>("voice_command"),
  voiceCancel: () => invoke<void>("voice_cancel"),
  voicePtt: (active: boolean) => invoke<void>("voice_ptt", { active }),
  voiceSpeak: (text: string) => invoke<void>("voice_speak", { text }),
  voiceStop: () => invoke<void>("voice_stop"),
  voiceHotkeySet: (hotkey: string) => invoke<void>("voice_hotkey_set", { hotkey }),
  islandInfo: () => invoke<{ present: boolean; width: number; height: number }>("island_info"),
  islandResize: (width: number, height: number) => invoke<void>("island_resize", { width, height }),
  companionSet: (visible: boolean) => invoke<void>("companion_set", { visible }),
  mainShow: () => invoke<void>("main_show"),
  permissions: () => invoke<Permissions>("permissions_status"),
  accounts: () => invoke<Account[]>("accounts_list"),
  oauthAvailable: () => invoke<("microsoft" | "google")[]>("oauth_available"),
  oauthConnect: (provider: "microsoft" | "google", teamsFor?: string) => invoke<Account>("oauth_connect", { provider, teamsFor }),
  oauthCancel: () => invoke<void>("oauth_cancel"),
  accountRemove: (id: string) => invoke<void>("account_remove", { id }),
  browserStatus: () => invoke<{ installed: string | null; running: boolean }>("browser_status"),
  browserClose: () => invoke<void>("browser_close"),
  browserResetProfile: () => invoke<void>("browser_reset_profile"),
  notifications: (hours = 48, appName?: string, query?: string) => invoke<Notifica[]>("notifications_list", { hours, appName, query }),
  notificationsMarkRead: () => invoke<number>("notifications_mark_read"),
  notificationSources: () => invoke<NotificationSources>("notifications_sources"),
  slackTokenSet: (token: string) => invoke<string>("slack_token_set", { token }),
  briefingNow: () => invoke<Run>("briefing_now"),
  briefingLatest: () => invoke<BriefingInfo | null>("briefing_latest"),
  cronPreview: (expr: string) => invoke<number[]>("cron_preview", { expr }),
  hotkeyCheck: (accelerator: string, agentId?: string) => invoke<string | null>("hotkey_check", { accelerator, agentId }),
  paletteOpenRun: (runId: string, conversationId: string, agentId: string) => invoke<void>("palette_open_run", { runId, conversationId, agentId }),
  mcpStatus: () => invoke<McpStatus[]>("mcp_status"),
  mcpReload: () => invoke<McpStatus[]>("mcp_reload"),
  mcpSecretSet: (id: string, kind: "headers" | "env", value: string) => invoke<void>("mcp_secret_set", { id, kind, value }),
  mcpSecretHas: (id: string) => invoke<{ headers: boolean; env: boolean }>("mcp_secret_has", { id }),
  mcpImportClaude: () => invoke<string[]>("mcp_import_claude"),
  devProjects: () => invoke<string[]>("dev_projects"),
  devOpen: (folder: string) => invoke<string>("dev_open", { folder }),
  devTree: (folder: string) => invoke<DevEntry[]>("dev_tree", { folder }),
  devFile: (path: string) => invoke<DevFile>("dev_file", { path }),
  devGit: (folder: string) => invoke<DevGit>("dev_git", { folder }),
  devDiff: (folder: string, file?: string) => invoke<string>("dev_diff", { folder, file }),
  devSession: (folder: string, session?: string) => invoke<DevSession>("dev_session", { folder, session }),
  devProcesses: () => invoke<DevProcess[]>("dev_processes"),
  devSend: (folder: string, message: string, opts: { session?: string; newSession?: boolean; here?: boolean } = {}) =>
    invoke<string>("dev_send", { folder, message, session: opts.session, newSession: opts.newSession, here: opts.here }),
  devStop: (folder: string) => invoke<boolean>("dev_stop", { folder }),
  editors: () => invoke<{ name: string; path: string }[]>("editors_list"),
  paletteToggle: () => invoke<void>("palette_toggle"),
  paletteHide: () => invoke<void>("palette_hide"),
  autostartSet: (enabled: boolean) => invoke<void>("autostart_set", { enabled }),
  requestPermission: (kind: "accessibility" | "screen" | "fullDisk" | "microphone") => invoke<void>("permissions_request", { kind }),
};

export function errorText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return "Qualcosa è andato storto";
}
