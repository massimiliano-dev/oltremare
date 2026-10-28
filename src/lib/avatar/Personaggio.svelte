<script lang="ts">
  // Il personaggio di Oltremare: un piccolo robot blu con una visiera scura e occhi luminosi.
  // Ogni stato ha una posa; la bocca segue il livello (voce reale o ritmo del testo).
  import type { AvatarState } from "../api";

  let { mood, level = 0, size = 200, track = true }: { mood: AvatarState; level?: number; size?: number; track?: boolean } = $props();

  const uid = $props.id();
  let root: SVGSVGElement | undefined = $state();
  let gaze = $state({ x: 0, y: 0 });
  let blink = $state(false);

  // Battito di ciglia casuale, solo quando gli occhi sono aperti
  $effect(() => {
    let t: ReturnType<typeof setTimeout>;
    const next = () => {
      t = setTimeout(() => {
        blink = true;
        setTimeout(() => (blink = false), 130);
        next();
      }, 2400 + Math.random() * 3600);
    };
    next();
    return () => clearTimeout(t);
  });

  // Lo sguardo segue il puntatore
  $effect(() => {
    if (!track) return;
    const move = (e: PointerEvent) => {
      if (!root) return;
      const r = root.getBoundingClientRect();
      const dx = e.clientX - (r.left + r.width / 2);
      const dy = e.clientY - (r.top + r.height * 0.5);
      const d = Math.hypot(dx, dy) || 1;
      const k = Math.min(1, d / 420);
      gaze = { x: (dx / d) * 5 * k, y: (dy / d) * 3.5 * k };
    };
    window.addEventListener("pointermove", move, { passive: true });
    return () => window.removeEventListener("pointermove", move);
  });

  const follows = $derived(mood === "idle" || mood === "listening" || mood === "speaking");
  const eyes = $derived(
    mood === "thinking" ? { x: -4, y: -5 } : mood === "working" ? { x: 0, y: 3 } : follows ? gaze : { x: 0, y: 0 },
  );
  const open = $derived(
    mood === "sleep" ? 0.08 :
    blink ? 0.1 :
    mood === "listening" ? 1.12 :
    mood === "alert" ? 1.25 :
    mood === "thinking" ? 0.72 :
    mood === "working" ? 0.55 : 1,
  );
  const mouth = $derived(0.12 + Math.min(1, level) * 0.88);
</script>

<svg bind:this={root} class="av s-{mood}" width={size} height={size} viewBox="0 0 200 200" role="img" aria-hidden="true">
  <defs>
    <linearGradient id="{uid}-body" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" style="stop-color: var(--av-top)" />
      <stop offset="1" style="stop-color: var(--av-bottom)" />
    </linearGradient>
    <linearGradient id="{uid}-visor" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" style="stop-color: var(--av-visor-top)" />
      <stop offset="1" style="stop-color: var(--av-visor)" />
    </linearGradient>
    <filter id="{uid}-glow" x="-60%" y="-60%" width="220%" height="220%">
      <feGaussianBlur stdDeviation="2.4" result="b" />
      <feMerge><feMergeNode in="b" /><feMergeNode in="SourceGraphic" /></feMerge>
    </filter>
  </defs>

  <ellipse class="shadow" cx="100" cy="186" rx="48" ry="6" />

  <g class="body">
    <!-- Antenna -->
    <line x1="100" y1="52" x2="100" y2="32" class="stem" />
    <circle class="bulb" cx="100" cy="27" r="7" filter="url(#{uid}-glow)" style:--lv={level} />

    <!-- Orecchie -->
    <rect class="ear" x="27" y="94" width="11" height="30" rx="5.5" />
    <rect class="ear" x="162" y="94" width="11" height="30" rx="5.5" />

    <!-- Testa -->
    <rect x="34" y="50" width="132" height="120" rx="56" fill="url(#{uid}-body)" />
    <path class="gloss" d="M58 62 C 80 54, 120 54, 142 62 C 130 58, 70 58, 58 62 Z" />
    <rect class="visor" x="50" y="74" width="100" height="76" rx="38" fill="url(#{uid}-visor)" />

    <!-- Guance -->
    <circle class="cheek" cx="66" cy="130" r="6" />
    <circle class="cheek" cx="134" cy="130" r="6" />

    <!-- Sopracciglia -->
    <g class="brows">
      <path class="brow l" d="M73 88 Q 82 84 91 88" />
      <path class="brow r" d="M109 88 Q 118 84 127 88" />
    </g>

    <!-- Occhi -->
    <g class="eyes" style:transform="translate({eyes.x}px, {eyes.y}px)" filter="url(#{uid}-glow)">
      {#if mood === "error"}
        <path class="x" d="M77 101 l10 10 M87 101 l-10 10 M113 101 l10 10 M123 101 l-10 10" />
      {:else}
        <rect class="eye" x="75" y="95" width="14" height="22" rx="7" style:transform="scaleY({open})" style:transform-origin="82px 106px" />
        <rect class="eye" x="111" y="95" width="14" height="22" rx="7" style:transform="scaleY({open})" style:transform-origin="118px 106px" />
      {/if}
    </g>

    <!-- Bocca -->
    <g class="mouth" filter="url(#{uid}-glow)">
      {#if mood === "speaking"}
        <ellipse cx="100" cy="132" rx="9" ry="7" style:transform="scaleY({mouth})" style:transform-origin="100px 132px" />
      {:else if mood === "asking"}
        <ellipse cx="100" cy="132" rx="4.5" ry="4" />
      {:else if mood === "alert"}
        <ellipse cx="100" cy="132" rx="6" ry="6" />
      {:else if mood === "thinking"}
        <path class="line" d="M96 132 h12" />
      {:else if mood === "sleep"}
        <path class="line" d="M93 132 h14" />
      {:else if mood === "error"}
        <path class="line" d="M87 134 q3.25 -4 6.5 0 t6.5 0 t6.5 0 t6.5 0" />
      {:else if mood === "working"}
        <g class="dots"><circle cx="92" cy="132" r="2.4" /><circle cx="100" cy="132" r="2.4" /><circle cx="108" cy="132" r="2.4" /></g>
      {:else}
        <path class="line" d="M90 129 Q 100 137 110 129" />
      {/if}
    </g>
  </g>

  <!-- Segni attorno alla testa -->
  {#if mood === "thinking"}
    <g class="thought"><circle cx="150" cy="44" r="3" /><circle cx="160" cy="34" r="4.2" /><circle cx="173" cy="24" r="5.6" /></g>
  {:else if mood === "asking"}
    <text class="q" x="158" y="46">?</text>
  {:else if mood === "sleep"}
    <g class="zz"><text x="146" y="48">z</text><text x="158" y="34">z</text></g>
  {:else if mood === "alert"}
    <g class="buzz"><path d="M83 20 q-7 6 0 12" /><path d="M117 20 q7 6 0 12" /></g>
  {:else if mood === "listening"}
    <g class="waves" style:--lv={level}><path d="M18 98 q-7 11 0 22" /><path d="M182 98 q7 11 0 22" /><path d="M10 92 q-10 17 0 34" /><path d="M190 92 q10 17 0 34" /></g>
  {/if}
</svg>

<style>
  .av {
    --av-top: oklch(0.6 0.19 var(--accent-h));
    --av-bottom: oklch(0.38 0.18 var(--accent-h));
    --av-visor-top: oklch(0.23 0.08 var(--accent-h));
    --av-visor: oklch(0.15 0.06 var(--accent-h));
    --av-glow: oklch(0.94 0.06 var(--accent-h));
    --av-bulb: oklch(0.84 0.13 var(--accent-h));
    --av-sign: var(--accent);
    display: block;
    overflow: visible;
  }
  .s-asking {
    --av-bulb: oklch(0.85 0.15 80);
    --av-sign: var(--warn);
  }
  .s-alert {
    --av-bulb: oklch(0.85 0.16 80);
    --av-sign: var(--warn);
  }
  .s-error {
    --av-bulb: oklch(0.7 0.18 25);
    --av-glow: oklch(0.86 0.09 25);
  }
  .s-sleep {
    --av-glow: oklch(0.7 0.06 var(--accent-h));
    --av-bulb: oklch(0.55 0.08 var(--accent-h));
  }

  .shadow {
    fill: var(--ink);
    opacity: 0.1;
    transform-origin: 100px 186px;
  }
  .body {
    transform-origin: 100px 170px;
    transition: transform 0.4s var(--ease-out);
  }
  .stem {
    stroke: var(--av-top);
    stroke-width: 5;
    stroke-linecap: round;
  }
  .bulb {
    fill: var(--av-bulb);
    transform-origin: 100px 27px;
    transition: fill 0.3s;
  }
  .ear {
    fill: var(--av-bottom);
  }
  .gloss {
    fill: oklch(1 0 0 / 0.22);
  }
  .cheek {
    fill: oklch(0.72 0.14 10);
    opacity: 0.3;
    transition: opacity 0.3s;
  }
  .s-speaking .cheek,
  .s-asking .cheek {
    opacity: 0.45;
  }
  .eye {
    fill: var(--av-glow);
    transition: transform 0.12s var(--ease-out), fill 0.3s;
  }
  .eyes {
    transition: transform 0.18s var(--ease-out);
  }
  .x,
  .line {
    fill: none;
    stroke: var(--av-glow);
    stroke-width: 3.4;
    stroke-linecap: round;
  }
  .mouth ellipse,
  .dots circle {
    fill: var(--av-glow);
  }
  .mouth ellipse {
    transition: transform 0.07s linear;
  }
  .brow {
    fill: none;
    stroke: var(--av-glow);
    stroke-width: 2.6;
    stroke-linecap: round;
    opacity: 0;
    transition: opacity 0.25s, transform 0.25s var(--ease-out);
  }
  .s-thinking .brow,
  .s-alert .brow {
    opacity: 0.8;
    transform: translateY(-3px);
  }
  .s-asking .brow.r {
    opacity: 0.85;
    transform: translateY(-5px) rotate(-6deg);
    transform-origin: 118px 86px;
  }
  .s-asking .brow.l {
    opacity: 0.6;
  }
  .s-error .brow.l {
    opacity: 0.8;
    transform: rotate(12deg);
    transform-origin: 82px 88px;
  }
  .s-error .brow.r {
    opacity: 0.8;
    transform: rotate(-12deg);
    transform-origin: 118px 88px;
  }

  .thought circle {
    fill: var(--av-sign);
    opacity: 0;
    animation: bubble 1.8s infinite;
  }
  .thought circle:nth-child(2) {
    animation-delay: 0.25s;
  }
  .thought circle:nth-child(3) {
    animation-delay: 0.5s;
  }
  .q {
    font-family: var(--font-serif);
    font-size: 34px;
    fill: var(--av-sign);
    animation: bob 1.6s ease-in-out infinite;
  }
  .zz text {
    font-family: var(--font-serif);
    font-style: italic;
    fill: var(--ink-3);
    font-size: 16px;
    animation: float 3s ease-in-out infinite;
  }
  .zz text:nth-child(2) {
    font-size: 21px;
    animation-delay: 1.2s;
  }
  .buzz path,
  .waves path {
    fill: none;
    stroke: var(--av-sign);
    stroke-width: 3;
    stroke-linecap: round;
  }
  .buzz path {
    animation: blinkfast 0.5s steps(2) infinite;
  }
  .waves path {
    opacity: calc(0.25 + var(--lv, 0) * 0.75);
    transition: opacity 0.08s;
  }
  .waves path:nth-child(n + 3) {
    opacity: calc(var(--lv, 0) * 0.8);
  }
  .s-listening .bulb {
    transform: scale(calc(1 + var(--lv, 0) * 0.35));
  }
  .s-speaking .bulb {
    transform: scale(calc(1 + var(--lv, 0) * 0.2));
  }

  /* Pose del corpo */
  .s-idle .body {
    animation: breathe 4.2s ease-in-out infinite;
  }
  .s-sleep .body {
    animation: breathe 6s ease-in-out infinite;
    transform: translateY(3px) rotate(-2deg);
  }
  .s-listening .body {
    transform: translateY(-3px) scale(1.015);
  }
  .s-thinking .body {
    animation: ponder 3.4s ease-in-out infinite;
  }
  .s-thinking .bulb {
    animation: glow 1.1s ease-in-out infinite;
  }
  .s-speaking .body {
    animation: talk 0.7s ease-in-out infinite;
  }
  .s-working .body {
    animation: hop 0.55s ease-in-out infinite;
  }
  .s-asking .body {
    transform: rotate(-7deg);
  }
  .s-alert .body {
    animation: jump 0.9s var(--ease-out) 2;
  }
  .s-alert .bulb {
    animation: glow 0.45s steps(2) infinite;
  }
  .s-error .body {
    animation: shake 0.5s ease-in-out 1;
  }

  @keyframes breathe {
    50% {
      transform: translateY(-1.8px) scaleY(1.006);
    }
  }
  @keyframes ponder {
    0%, 100% {
      transform: rotate(-3deg);
    }
    50% {
      transform: rotate(3deg) translateY(-1px);
    }
  }
  @keyframes talk {
    50% {
      transform: translateY(-1.5px) rotate(0.6deg);
    }
  }
  @keyframes hop {
    50% {
      transform: translateY(-4px);
    }
  }
  @keyframes jump {
    30% {
      transform: translateY(-12px) scale(1.03, 0.98);
    }
    55% {
      transform: translateY(0) scale(1.04, 0.95);
    }
    75% {
      transform: translateY(-3px);
    }
  }
  @keyframes shake {
    20%, 60% {
      transform: translateX(-4px);
    }
    40%, 80% {
      transform: translateX(4px);
    }
  }
  @keyframes glow {
    50% {
      opacity: 0.55;
    }
  }
  @keyframes bubble {
    0% {
      opacity: 0;
      transform: translateY(3px);
    }
    30%, 70% {
      opacity: 0.85;
      transform: none;
    }
    100% {
      opacity: 0;
    }
  }
  @keyframes bob {
    50% {
      transform: translateY(-3px);
    }
  }
  @keyframes float {
    0% {
      opacity: 0;
      transform: translate(0, 4px);
    }
    40% {
      opacity: 1;
    }
    100% {
      opacity: 0;
      transform: translate(6px, -10px);
    }
  }
  @keyframes blinkfast {
    50% {
      opacity: 0.2;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .body,
    .bulb,
    .thought circle,
    .q,
    .zz text,
    .buzz path {
      animation: none !important;
    }
    .thought circle,
    .zz text {
      opacity: 0.8;
    }
  }
</style>
