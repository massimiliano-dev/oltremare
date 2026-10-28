<script lang="ts">
  // Il volto astratto: una sfera luminosa con tre anelli d'onda.
  // L'ampiezza segue il livello della voce; colore e ritmo seguono lo stato.
  import type { AvatarState } from "../api";

  let { mood, level = 0, size = 200 }: { mood: AvatarState; level?: number; size?: number } = $props();

  let canvas: HTMLCanvasElement | undefined = $state();

  // oklch → sRGB (0..255), per disegnare con i colori del tema
  function oklch(l: number, c: number, h: number): [number, number, number] {
    const hr = (h * Math.PI) / 180;
    const a = c * Math.cos(hr);
    const b = c * Math.sin(hr);
    const l_ = (l + 0.3963377774 * a + 0.2158037573 * b) ** 3;
    const m_ = (l - 0.1055613458 * a - 0.0638541728 * b) ** 3;
    const s_ = (l - 0.0894841775 * a - 1.291485548 * b) ** 3;
    const lin = [
      4.0767416621 * l_ - 3.3077115913 * m_ + 0.2309699292 * s_,
      -1.2684380046 * l_ + 2.6097574011 * m_ - 0.3413193965 * s_,
      -0.0041960863 * l_ - 0.7034186147 * m_ + 1.707614701 * s_,
    ];
    return lin.map((x) => {
      const v = x <= 0.0031308 ? 12.92 * x : 1.055 * Math.pow(Math.max(0, x), 1 / 2.4) - 0.055;
      return Math.round(Math.min(1, Math.max(0, v)) * 255);
    }) as [number, number, number];
  }
  const rgba = (c: [number, number, number], a: number) => `rgba(${c[0]},${c[1]},${c[2]},${a})`;

  // Per stato: tonalità (null = accento), ampiezza a riposo, velocità, lobi, luminosità
  const LOOK: Record<AvatarState, { hue: number | null; amp: number; speed: number; lobes: number; bright: number }> = {
    sleep: { hue: null, amp: 0.01, speed: 0.15, lobes: 3, bright: 0.45 },
    idle: { hue: null, amp: 0.025, speed: 0.4, lobes: 3, bright: 0.85 },
    listening: { hue: null, amp: 0.03, speed: 0.9, lobes: 5, bright: 1 },
    thinking: { hue: null, amp: 0.05, speed: 2.2, lobes: 7, bright: 0.95 },
    speaking: { hue: null, amp: 0.03, speed: 1.2, lobes: 4, bright: 1 },
    working: { hue: null, amp: 0.04, speed: 1.8, lobes: 6, bright: 0.95 },
    asking: { hue: 78, amp: 0.04, speed: 0.9, lobes: 4, bright: 1 },
    alert: { hue: 78, amp: 0.06, speed: 2.6, lobes: 5, bright: 1 },
    error: { hue: 25, amp: 0.05, speed: 1.4, lobes: 9, bright: 0.9 },
  };

  let lv = 0;
  let amp = 0.025;
  let speed = 0.4;
  let bright = 0.85;
  let hue = 275;
  let lobes = 3;
  /** 0 = accento blu, 1 = stati caldi: più chiari e meno saturi per restare nel gamut */
  let warm = 0;

  $effect(() => {
    const cv = canvas;
    if (!cv) return;
    const ctx = cv.getContext("2d")!;
    const reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;
    let raf = 0;
    let phase = 0;
    let last = performance.now();

    const draw = (now: number) => {
      const dt = Math.min(0.05, (now - last) / 1000);
      last = now;
      const dpr = window.devicePixelRatio || 1;
      const px = Math.round(size * dpr);
      if (cv.width !== px) {
        cv.width = px;
        cv.height = px;
      }
      const accent = Number(getComputedStyle(cv).getPropertyValue("--accent-h")) || 275;
      const look = LOOK[mood];
      // Avvicinamento morbido ai valori dello stato
      const k = 1 - Math.pow(0.002, dt);
      lv += (level - lv) * Math.min(1, k * 4);
      amp += (look.amp - amp) * k;
      speed += (look.speed - speed) * k;
      bright += (look.bright - bright) * k;
      const targetHue = look.hue ?? accent;
      hue += (((targetHue - hue + 540) % 360) - 180) * k;
      lobes = look.lobes;
      warm += ((look.hue === null ? 0 : 1) - warm) * k;
      phase += dt * speed * (reduced ? 0.25 : 1);

      const S = cv.width;
      const c = S / 2;
      const R = S * 0.27;
      ctx.clearRect(0, 0, S, S);

      // Alone
      const pulse = mood === "alert" ? 0.75 + 0.25 * Math.sin(phase * 6) : 1;
      const halo = ctx.createRadialGradient(c, c, R * 0.6, c, c, R * 1.9);
      halo.addColorStop(0, rgba(oklch(0.72, 0.17, hue), 0.42 * bright * pulse + lv * 0.25));
      halo.addColorStop(1, rgba(oklch(0.72, 0.17, hue), 0));
      ctx.fillStyle = halo;
      ctx.fillRect(0, 0, S, S);

      // Sfera
      const breathe = 1 + Math.sin(phase * 1.3) * 0.012 + lv * 0.06;
      const r = R * breathe;
      const core = ctx.createRadialGradient(c - r * 0.35, c - r * 0.4, r * 0.05, c, c, r);
      core.addColorStop(0, rgba(oklch(0.95, 0.05, hue), bright));
      core.addColorStop(0.45, rgba(oklch(0.62 + warm * 0.16, 0.2 - warm * 0.05, hue), bright));
      core.addColorStop(1, rgba(oklch(0.32 + warm * 0.2, 0.17 - warm * 0.04, hue), bright));
      ctx.fillStyle = core;
      ctx.beginPath();
      ctx.arc(c, c, r, 0, Math.PI * 2);
      ctx.fill();

      // Anelli d'onda
      for (let i = 0; i < 3; i++) {
        const base = R * (1.16 + i * 0.13);
        const a = R * (amp + lv * 0.22) * (1 - i * 0.2);
        ctx.beginPath();
        for (let s = 0; s <= 120; s++) {
          const t = (s / 120) * Math.PI * 2;
          const w = Math.sin(t * (lobes + i) + phase * (1.6 + i * 0.7) * (i % 2 ? -1 : 1)) + 0.5 * Math.sin(t * 2 - phase * 0.9);
          const rr = base + a * w;
          const x = c + Math.cos(t) * rr;
          const y = c + Math.sin(t) * rr;
          s ? ctx.lineTo(x, y) : ctx.moveTo(x, y);
        }
        ctx.closePath();
        ctx.strokeStyle = rgba(oklch(0.8, 0.13, hue + i * 8), (0.55 - i * 0.15) * bright);
        ctx.lineWidth = Math.max(1, S * 0.006);
        ctx.stroke();
      }
      raf = requestAnimationFrame(draw);
    };
    raf = requestAnimationFrame(draw);
    return () => cancelAnimationFrame(raf);
  });
</script>

<canvas bind:this={canvas} style:width="{size}px" style:height="{size}px" aria-hidden="true"></canvas>

<style>
  canvas {
    display: block;
  }
</style>
