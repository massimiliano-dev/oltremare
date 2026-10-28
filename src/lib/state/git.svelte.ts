// Git dello Studio: stato (pronte / modificate), azioni, rami, sincronizzazione, differenze.
import { api, errorText, type GitState } from "../api";
import { dev } from "./dev.svelte";
import { ui } from "./ui.svelte";

class GitStore {
  state = $state<GitState | null>(null);
  project = $state<string | null>(null);
  message = $state("");
  busy = $state<string | null>(null);
  /** Differenze mostrate: di un file o di tutti, pronte o no */
  diff = $state("");
  diffFile = $state<string | null>(null);
  diffStaged = $state(false);
  view = $state<"unified" | "split">((typeof localStorage !== "undefined" && (localStorage.getItem("oltremare:diff-view") as "unified" | "split")) || "unified");
  private timer: ReturnType<typeof setInterval> | null = null;

  staged = $derived(this.state?.staged ?? []);
  changed = $derived(this.state?.changed ?? []);

  start(project: string) {
    this.stop();
    this.project = project;
    this.message = "";
    this.refresh();
    this.timer = setInterval(() => this.refresh(), 4000);
  }

  stop() {
    if (this.timer) clearInterval(this.timer);
    this.timer = null;
  }

  async refresh() {
    if (!this.project) return;
    this.state = await api.gitState(this.project).catch(() => null);
  }

  setView(v: "unified" | "split") {
    this.view = v;
    try {
      localStorage.setItem("oltremare:diff-view", v);
    } catch {}
  }

  async showDiff(file: string | null, staged: boolean) {
    if (!this.project) return;
    this.diffFile = file;
    this.diffStaged = staged;
    dev.tab = "diff";
    let d = await api.gitDiff(this.project, staged, file ?? undefined).catch((e) => errorText(e));
    // `git diff` non include i file nuovi non tracciati: si aggiungono uno per uno
    if (!file && !staged) {
      const nuovi = this.changed.filter((f) => f.status === "?").slice(0, 20);
      const extra = await Promise.all(nuovi.map((f) => api.gitDiff(this.project!, false, f.path).catch(() => "")));
      d = [d, ...extra].filter((x) => x.trim()).join("\n");
    }
    this.diff = d;
  }

  private async run<T>(label: string, f: () => Promise<T>, ok?: (r: T) => string | void) {
    if (!this.project || this.busy) return;
    this.busy = label;
    try {
      const r = await f();
      const msg = ok?.(r);
      if (msg) ui.toast(msg);
    } catch (e) {
      ui.toast(errorText(e), "error", undefined, 9000);
    } finally {
      this.busy = null;
      await this.refresh();
      dev.refreshGit();
      if (dev.tab === "diff") this.showDiff(this.diffFile, this.diffStaged);
    }
  }

  stage(files: string[]) {
    return this.run("stage", () => api.gitStage(this.project!, files));
  }
  unstage(files: string[]) {
    return this.run("unstage", () => api.gitUnstage(this.project!, files));
  }
  discard(files: string[]) {
    return this.run("discard", () => api.gitDiscard(this.project!, files), () => (files.length === 1 ? "Modifiche annullate" : `${files.length} file ripristinati`));
  }
  commit(opts: { amend?: boolean; push?: boolean } = {}) {
    const all = !opts.amend && !this.staged.length;
    return this.run(
      "commit",
      async () => {
        const r = await api.gitCommit(this.project!, this.message, { amend: opts.amend, all });
        if (opts.push) await api.gitSync(this.project!, "push");
        return r;
      },
      (r) => {
        this.message = "";
        return `Commit ${r}${opts.push ? " · inviato" : ""}`;
      },
    );
  }
  sync(action: "pull" | "push" | "fetch") {
    const msg = { pull: "Aggiornato dal remoto", push: "Inviato al remoto", fetch: "Remoto controllato" };
    return this.run(action, () => api.gitSync(this.project!, action), () => msg[action]);
  }
  checkout(branch: string, create = false) {
    return this.run("checkout", () => api.gitCheckout(this.project!, branch, create), () => (create ? `Nuovo ramo ${branch}` : `Sei su ${branch}`));
  }
  stash(pop = false) {
    return this.run("stash", () => api.gitStash(this.project!, pop), () => (pop ? "Modifiche riprese dallo stash" : "Modifiche messe da parte"));
  }
  async suggest() {
    if (!this.project || this.busy) return;
    this.busy = "suggest";
    try {
      this.message = await api.gitSuggestMessage(this.project);
    } catch (e) {
      ui.toast(errorText(e), "error");
    } finally {
      this.busy = null;
    }
  }
}

export const git = new GitStore();
