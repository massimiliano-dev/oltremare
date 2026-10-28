<script lang="ts">
  // L'isola: Oltremare in una pillola nera in alto, attorno alla notch (come la Dynamic Island).
  // A riposo è piccola; si allarga quando ascolta, lavora, parla, chiede o avvisa.
  import { onMount } from "svelte";
  import Icon from "./lib/Icon.svelte";
  import Personaggio from "./lib/avatar/Personaggio.svelte";
  import { api, type Agent, type ApprovalRequest, type AvatarState, type Notifica } from "./lib/api";
  import { on } from "./lib/events";

  type Mode = "idle" | "hover" | "listening" | "thinking" | "speaking" | "asking" | "alert" | "error";
  let notch = $state({ present: false, width: 0, height: 34 });
  let mood = $state<AvatarState>("idle");
  let level = $state(0);
  let transcript = $state("");
  let recording = $state(false);
  let activity = $state("");
  let agentName = $state("");
  let spoken = $state("");
  let approvals = $state<ApprovalRequest[]>([]);
  let alert = $state<Notifica | null>(null);
  let hovering = $state(false);
  let agents: Agent[] = [];
  let alertTimer: ReturnType<typeof setTimeout> | null = null;
  let hoverTimer: ReturnType<typeof setTimeout> | null = null;

  const approval = $derived(approvals[0] ?? null);
  const mode = $derived<Mode>(
    approval ? "asking"
    : alert ? "alert"
    : recording || mood === "listening" ? "listening"
    : mood === "speaking" ? "speaking"
    : mood === "thinking" || mood === "working" ? "thinking"
    : mood === "error" ? "error"
    : hovering ? "hover"
    : "idle",
  );

  /** Quando l'isola è aperta il robottino passa dall'ala al contenuto, più grande */
  const open = $derived(mode !== "idle" && mode !== "hover");

  // Misure della pillola per ogni modo (in punti). Con la notch la prima riga resta libera per la fotocamera.
  const top = $derived(notch.present ? notch.height : 0);
  const size = $derived.by(() => {
    const base = notch.present ? notch.width + 80 : 190;
    const wide = Math.max(base + 200, 400);
    switch (mode) {
      case "idle":
        return { w: base, h: notch.present ? notch.height : 34 };
      case "hover":
        return { w: Math.max(base + 120, 320), h: top + 52 };
      case "asking":
        return { w: Math.max(wide, 460), h: top + 112 };
      case "alert":
        return { w: wide, h: top + 64 };
      default:
        return { w: wide, h: top + 58 };
    }
  });

  // La finestra segue la pillola: si allarga subito, si stringe dopo l'animazione
  let applied = { w: 0, h: 0 };
  let shrinkTimer: ReturnType<typeof setTimeout> | null = null;
  $effect(() => {
    const { w, h } = size;
    const pad = 16; // spazio per l'ombra
    if (shrinkTimer) clearTimeout(shrinkTimer);
    if (w >= applied.w && h >= applied.h) {
      applied = { w, h };
      api.islandResize(w + pad * 2, h + pad).catch(() => {});
    } else {
      // Prima si anima, poi la finestra si adegua
      api.islandResize(Math.max(w, applied.w) + pad * 2, Math.max(h, applied.h) + pad).catch(() => {});
      shrinkTimer = setTimeout(() => {
        applied = { w, h };
        api.islandResize(w + pad * 2, h + pad).catch(() => {});
      }, 420);
    }
  });

  const bars = $derived(Array.from({ length: 18 }, (_, i) => 0.18 + Math.abs(Math.sin(i * 1.7 + level * 9)) * level * 0.95));

  onMount(() => {
    document.documentElement.classList.add("island");
    api.islandInfo().then((n) => (notch = n)).catch(() => {});
    api.agents().then((l) => (agents = l)).catch(() => {});
    api.approvalsPending().then((l) => (approvals = l)).catch(() => {});
    api.avatarState().then((s) => (mood = s)).catch(() => {});
    on("avatar-state", (p) => {
      mood = p.state;
      level = p.level;
    });
    on("voice-status", (p) => (recording = p.recording));
    on("voice-transcript", (p) => {
      transcript = p.text;
      if (p.final) setTimeout(() => (transcript = ""), 3500);
    });
    on("run-started", ({ run }) => {
      agentName = agents.find((a) => a.id === run.agentId)?.name ?? "Oltremare";
      activity = "Ci penso…";
      spoken = "";
    });
    on("run-progress", (p) => (activity = p.text));
    on("run-step", ({ step }) => {
      if (step.kind === "tool" && step.title) activity = step.title;
    });
    on("run-delta", (p) => {
      if (p.kind === "text") {
        spoken = (spoken + p.delta).slice(-220);
        activity = "";
      }
    });
    on("run-done", () => {
      activity = "";
      setTimeout(() => (spoken = ""), 4000);
    });
    on("run-approval", ({ request }) => (approvals = [...approvals.filter((a) => a.id !== request.id), request]));
    on("run-approval-resolved", (p) => (approvals = approvals.filter((a) => a.id !== p.approvalId)));
    on("notification-alert", ({ notification }) => {
      alert = notification;
      if (alertTimer) clearTimeout(alertTimer);
      alertTimer = setTimeout(() => (alert = null), 6000);
    });
    on("agents-changed", () => api.agents().then((l) => (agents = l)));
  });

  function enter() {
    if (hoverTimer) clearTimeout(hoverTimer);
    hoverTimer = setTimeout(() => (hovering = true), 120);
  }
  function leave() {
    if (hoverTimer) clearTimeout(hoverTimer);
    hoverTimer = setTimeout(() => (hovering = false), 450);
  }

  function clickPill() {
    if (mode === "idle" || mode === "hover") api.paletteToggle().catch(() => {});
    else if (mode === "alert") {
      alert = null;
      api.mainShow();
    } else if (mode === "thinking" || mode === "speaking") api.mainShow();
  }
</script>

<div class="stage">
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div
    class="pill m-{mode}"
    class:notch={notch.present}
    style="width:{size.w}px;height:{size.h}px;--top:{top}px"
    onmouseenter={enter}
    onmouseleave={leave}
    onclick={clickPill}
  >
    <!-- Ali ai lati della notch: sempre visibili -->
    <div class="wings" style="height:{notch.present ? notch.height : 34}px">
      <span class="bot" class:hidden={open}><Personaggio mood={mood} {level} size={notch.present ? Math.min(28, notch.height - 2) : 28} track={false} /></span>
      <span class="status">
        {#if mode === "listening"}<span class="rec"></span>
        {:else if mode === "thinking"}<span class="spin"></span>
        {:else if mode === "speaking"}<span class="eq">{#each [0, 1, 2] as i (i)}<i style="height:{4 + level * 12 * (1 - i * 0.2)}px"></i>{/each}</span>
        {:else if mode === "asking"}<span class="q">?</span>
        {:else if mode === "alert"}<Icon name="bell" size={13} />
        {:else if !notch.present}<span class="word">Oltremare</span>{/if}
      </span>
    </div>

    <div class="body">
      {#if open}<span class="bigbot"><Personaggio mood={mood} {level} size={44} track={false} /></span>{/if}
      {#if mode === "listening"}
        <div class="row">
          <span class="wave">{#each bars as b, i (i)}<i style="transform:scaleY({b})"></i>{/each}</span>
          <span class="txt">{transcript || "Ti ascolto…"}</span>
        </div>
      {:else if mode === "thinking"}
        <div class="row">
          <strong>{agentName || "Oltremare"}</strong>
          <span class="txt dim">{activity || "Al lavoro"}</span>
        </div>
      {:else if mode === "speaking"}
        <div class="row">
          <span class="wave">{#each bars as b, i (i)}<i style="transform:scaleY({b})"></i>{/each}</span>
          <span class="txt">{spoken.split(/(?<=[.!?])\s/).pop() || "…"}</span>
        </div>
      {:else if mode === "asking" && approval}
        <div class="ask">
          <span class="txt"><strong>{approval.agentName}</strong> chiede: {approval.title}</span>
          <small>{approval.summary}</small>
          <div class="btns">
            <button class="no" onclick={(e) => { e.stopPropagation(); api.approvalAnswer(approval.id, "deny"); }}>Nega</button>
            <button class="no" onclick={(e) => { e.stopPropagation(); api.approvalAnswer(approval.id, "allowAlways"); }} title="Non chiedere più per questa azione">Sempre</button>
            <button class="yes" onclick={(e) => { e.stopPropagation(); api.approvalAnswer(approval.id, "allow"); }}>Consenti</button>
          </div>
        </div>
      {:else if mode === "alert" && alert}
        <div class="row">
          <strong>{alert.app}</strong>
          <span class="txt">{alert.title}{alert.body ? ` — ${alert.body}` : ""}</span>
        </div>
      {:else if mode === "error"}
        <div class="row"><span class="txt">Qualcosa non è andato: apri Oltremare per i dettagli</span></div>
      {:else if mode === "hover"}
        <div class="acts">
          <button onclick={(e) => { e.stopPropagation(); api.voiceCommand().catch(() => {}); }}><Icon name="mic" size={15} /> Parla</button>
          <button onclick={(e) => { e.stopPropagation(); api.paletteToggle().catch(() => {}); }}><Icon name="command" size={15} /> Chiedi</button>
          <button onclick={(e) => { e.stopPropagation(); api.mainShow(); }}><Icon name="external" size={15} /> Apri</button>
        </div>
      {/if}
    </div>
  </div>
</div>

<style>
  :global(html.island),
  :global(html.island body),
  :global(html.island #app) {
    background: transparent !important;
    overflow: hidden;
  }
  .stage {
    display: flex;
    justify-content: center;
    height: 100vh;
  }
  .pill {
    position: relative;
    background: #000;
    color: #f4f4f6;
    border-radius: 0 0 22px 22px;
    overflow: hidden;
    cursor: default;
    transition:
      width 0.42s cubic-bezier(0.32, 1.25, 0.55, 1),
      height 0.42s cubic-bezier(0.32, 1.25, 0.55, 1),
      border-radius 0.3s ease;
    box-shadow: 0 8px 24px -8px rgba(0, 0, 0, 0.55);
    font-size: 13px;
    -webkit-font-smoothing: antialiased;
  }
  .pill:not(.notch) {
    border-radius: 999px;
  }
  .pill:not(.notch):not(.m-idle) {
    border-radius: 26px;
  }
  .pill.m-idle {
    box-shadow: none;
  }
  .wings {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 14px;
  }
  .bot {
    display: grid;
    place-items: center;
    width: 28px;
    margin-left: -4px;
    transition: opacity 0.2s, transform 0.3s cubic-bezier(0.32, 1.25, 0.55, 1);
  }
  .bot.hidden {
    opacity: 0;
    transform: scale(0.6) translateY(6px);
  }
  .bigbot {
    display: grid;
    place-items: center;
    flex: none;
    width: 44px;
    margin: 0 10px 0 -6px;
    animation: pop 0.38s cubic-bezier(0.32, 1.35, 0.55, 1) both;
  }
  @keyframes pop {
    from {
      transform: scale(0.5) translateY(-10px);
      opacity: 0;
    }
  }
  .orb {
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: radial-gradient(circle at 35% 30%, oklch(0.82 0.12 275), oklch(0.5 0.22 275) 60%, oklch(0.32 0.16 275));
    box-shadow: 0 0 calc(4px + var(--lv) * 14px) oklch(0.6 0.2 275 / 0.8);
    transform: scale(calc(1 + var(--lv) * 0.25));
    transition: transform 0.08s, box-shadow 0.08s;
    animation: breathe 3.6s ease-in-out infinite;
  }
  .orb.s-asking {
    background: radial-gradient(circle at 35% 30%, oklch(0.92 0.12 90), oklch(0.75 0.17 75));
  }
  .orb.s-alert {
    animation: blink 0.9s ease-in-out infinite;
  }
  .orb.s-error {
    background: radial-gradient(circle at 35% 30%, oklch(0.8 0.12 25), oklch(0.58 0.2 25));
  }
  @keyframes breathe {
    50% {
      filter: brightness(1.25);
    }
  }
  @keyframes blink {
    50% {
      opacity: 0.35;
    }
  }
  .status {
    display: flex;
    align-items: center;
    min-width: 16px;
    justify-content: flex-end;
    color: oklch(0.78 0.12 275);
  }
  .word {
    font-family: var(--font-serif);
    font-style: italic;
    font-size: 15px;
    color: #f4f4f6;
  }
  .rec {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: #ff453a;
    animation: blink 1.2s infinite;
  }
  .spin {
    width: 13px;
    height: 13px;
    border-radius: 50%;
    border: 2px solid rgba(255, 255, 255, 0.18);
    border-top-color: oklch(0.78 0.14 275);
    animation: s 0.8s linear infinite;
  }
  @keyframes s {
    to {
      transform: rotate(360deg);
    }
  }
  .eq {
    display: flex;
    gap: 2px;
    align-items: center;
  }
  .eq i {
    width: 3px;
    border-radius: 2px;
    background: oklch(0.78 0.14 275);
    transition: height 0.08s;
  }
  .q {
    font-weight: 800;
    color: oklch(0.85 0.15 85);
  }
  .body {
    position: absolute;
    left: 0;
    right: 0;
    top: var(--top);
    bottom: 0;
    display: flex;
    align-items: center;
    padding: 0 18px;
    opacity: 1;
    animation: fadein 0.32s 0.08s both;
  }
  .pill:not(.notch) .body {
    top: 0;
    padding-left: 14px;
  }
  .pill:not(.notch).m-asking .body,
  .pill:not(.notch).m-hover .body {
    top: 30px;
    padding-left: 18px;
  }
  .m-idle .body {
    display: none;
  }
  @keyframes fadein {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
    width: 100%;
  }
  .row strong {
    flex: none;
    font-weight: 650;
  }
  .txt {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dim {
    color: rgba(244, 244, 246, 0.65);
  }
  .wave {
    display: flex;
    align-items: center;
    gap: 2px;
    height: 22px;
    flex: none;
  }
  .wave i {
    width: 3px;
    height: 22px;
    border-radius: 2px;
    background: oklch(0.75 0.15 275);
    transform-origin: center;
    transition: transform 0.08s;
  }
  .ask {
    display: grid;
    gap: 4px;
    width: 100%;
    min-width: 0;
  }
  .ask small {
    color: rgba(244, 244, 246, 0.6);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 12px;
  }
  .btns {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 4px;
  }
  .btns button {
    height: 28px;
    padding: 0 14px;
    border-radius: 999px;
    font-weight: 600;
    font-size: 12.5px;
  }
  .no {
    background: rgba(255, 255, 255, 0.12);
    color: #f4f4f6;
  }
  .yes {
    background: oklch(0.6 0.2 275);
    color: white;
  }
  .acts {
    display: flex;
    gap: 6px;
    width: 100%;
    justify-content: center;
  }
  .acts button {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 30px;
    padding: 0 12px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.1);
    color: #f4f4f6;
    font-size: 12.5px;
  }
  .acts button:hover {
    background: rgba(255, 255, 255, 0.18);
  }
  @media (prefers-reduced-motion: reduce) {
    .pill {
      transition: none;
    }
    .bot {
    display: grid;
    place-items: center;
    width: 28px;
    margin-left: -4px;
    transition: opacity 0.2s, transform 0.3s cubic-bezier(0.32, 1.25, 0.55, 1);
  }
  .bot.hidden {
    opacity: 0;
    transform: scale(0.6) translateY(6px);
  }
  .bigbot {
    display: grid;
    place-items: center;
    flex: none;
    width: 44px;
    margin: 0 10px 0 -6px;
    animation: pop 0.38s cubic-bezier(0.32, 1.35, 0.55, 1) both;
  }
  @keyframes pop {
    from {
      transform: scale(0.5) translateY(-10px);
      opacity: 0;
    }
  }
  .orb {
      animation: none;
    }
  }
</style>
