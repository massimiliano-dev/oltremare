import { api, errorText, type McpStatus } from "../api";
import { on } from "../events";
import { ui } from "./ui.svelte";

class McpStore {
  status = $state<McpStatus[]>([]);
  reloading = $state(false);

  async init() {
    on("mcp-status", (s) => (this.status = s));
    this.status = await api.mcpStatus().catch(() => []);
  }

  name(serverId: string) {
    return this.status.find((s) => s.id === serverId)?.name ?? "Server esterno";
  }

  async reload() {
    this.reloading = true;
    try {
      this.status = await api.mcpReload();
    } catch (e) {
      ui.toast(errorText(e), "error");
    } finally {
      this.reloading = false;
    }
  }
}

export const mcp = new McpStore();
