// Account collegati (Microsoft, Google) per posta, calendario, attività e Teams.
import { api, errorText, type Account } from "../api";
import { on } from "../events";
import { ui } from "./ui.svelte";

class AccountsState {
  list = $state<Account[]>([]);
  available = $state<("microsoft" | "google")[]>([]);
  connecting = $state<string | null>(null);
  url = $state<string | null>(null);

  async init() {
    on("accounts-changed", () => this.refresh());
    on("oauth-url", (u) => (this.url = u));
    await this.refresh();
  }

  async refresh() {
    const [l, a] = await Promise.all([api.accounts().catch(() => []), api.oauthAvailable().catch(() => [])]);
    this.list = l;
    this.available = a;
  }

  async connect(provider: "microsoft" | "google", teamsFor?: string) {
    this.connecting = teamsFor ? `teams:${teamsFor}` : provider;
    this.url = null;
    try {
      const a = await api.oauthConnect(provider, teamsFor);
      ui.toast(teamsFor ? `Teams collegato per ${a.email}` : `${a.email} collegato`);
      await this.refresh();
    } catch (e) {
      const m = errorText(e);
      if (!m.startsWith("Accesso annullato")) ui.toast(m, "error");
    } finally {
      this.connecting = null;
      this.url = null;
    }
  }

  cancel() {
    api.oauthCancel().catch(() => {});
    this.connecting = null;
  }

  async remove(a: Account) {
    await api.accountRemove(a.id).catch((e) => ui.toast(errorText(e), "error"));
    await this.refresh();
  }
}

export const accounts = new AccountsState();
