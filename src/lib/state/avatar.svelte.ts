// Stato dell'avatar: arriva dal backend (unico per tutta l'app). Il "labiale" quando non c'è
// audio segue il ritmo del testo in arrivo; con la voce userà il livello reale.
import { api, type AvatarState } from "../api";
import { on } from "../events";
import { settings } from "./settings.svelte";

const SLEEP_AFTER = 5 * 60_000;

export const STATE_LABEL: Record<AvatarState, string> = {
  sleep: "A riposo",
  idle: "Pronto",
  listening: "Ti ascolto",
  thinking: "Sto pensando",
  speaking: "Ti rispondo",
  working: "Al lavoro",
  asking: "Ho bisogno di te",
  alert: "Guarda qui",
  error: "Qualcosa è andato storto",
};

class AvatarStore {
  state = $state<AvatarState>("idle");
  /** Livello audio reale 0..1 (voce, dalla fase 4) */
  audio = $state(0);
  /** Livello simulato dal testo in streaming */
  private speech = $state(0);
  level = $derived(Math.max(this.audio, this.speech));
  face = $derived(settings.s?.avatarFace ?? "character");
  private idleSince = $state(Date.now());
  private now = $state(Date.now());
  shown = $derived<AvatarState>(this.state === "idle" && this.now - this.idleSince > SLEEP_AFTER ? "sleep" : this.state);
  label = $derived(STATE_LABEL[this.shown]);

  private raf = 0;
  private target = 0;

  async listen() {
    on("avatar-state", (p) => {
      this.state = p.state;
      this.audio = p.level;
      this.wake();
    });
    this.state = await api.avatarState().catch(() => "idle" as AvatarState);
    for (const ev of ["pointerdown", "keydown"]) window.addEventListener(ev, () => this.wake(), { passive: true });
    setInterval(() => (this.now = Date.now()), 20_000);
  }

  wake() {
    this.idleSince = Date.now();
    this.now = this.idleSince;
  }

  /** Un pezzo di testo è arrivato: la bocca si apre e poi si richiude piano. */
  pulse() {
    this.target = Math.min(1, 0.45 + Math.random() * 0.55);
    if (!this.raf) this.raf = requestAnimationFrame(this.tick);
  }

  private tick = () => {
    // Sale veloce verso il bersaglio, poi decade: sembra sillabare
    this.speech += (this.target - this.speech) * 0.45;
    this.target *= 0.8;
    if (this.speech < 0.02 && this.target < 0.02) {
      this.speech = 0;
      this.raf = 0;
      return;
    }
    this.raf = requestAnimationFrame(this.tick);
  };
}

export const avatar = new AvatarStore();
