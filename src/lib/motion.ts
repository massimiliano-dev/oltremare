// Transizioni di Oltremare: brevi, decelerate, e rispettose del "movimento ridotto".
import type { TransitionConfig } from "svelte/transition";

const reduced = () => typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;

// Decelerazione esponenziale, la stessa curva di --ease-ink
const easeInk = (t: number) => (t === 1 ? 1 : 1 - Math.pow(2, -10 * t));
const easeIn = (t: number) => t * t;

/**
 * Riga della lista che entra o esce: scivola di lato e richiude lo spazio,
 * così le altre righe salgono senza scatti (archivia, elimina, annulla, nuova posta).
 */
export function collapse(node: HTMLElement, { enabled = true, duration = 260 } = {}): TransitionConfig {
  if (!enabled) return { duration: 0 };
  const h = node.offsetHeight;
  if (reduced()) return { duration: 160, css: (t) => `opacity:${t}` };
  return {
    duration,
    easing: easeInk,
    css: (t, u) =>
      `overflow:hidden;height:${t * h}px;opacity:${Math.min(1, t * 1.6)};` +
      `transform:translateX(${u * 28}px);margin-block:${-u * 0.5}px`,
  };
}

/** Pannelli che salgono dal basso (composer) o arrivano di lato (assistente, impostazioni). */
export function sheet(
  _node: Element,
  { x = 0, y = 0, scale = 1, duration = 200 }: { x?: number; y?: number; scale?: number; duration?: number } = {},
): TransitionConfig {
  if (reduced()) return { duration: 140, css: (t) => `opacity:${t}` };
  return {
    duration,
    easing: easeIn,
    css: (t, u) => `opacity:${t};transform:translate(${u * x}px,${u * y}px) scale(${1 - u * (1 - scale)})`,
  };
}

/** Dissolvenza semplice per sfondi e popover. */
export function fadeOut(_node: Element, { duration = 140 } = {}): TransitionConfig {
  return { duration, css: (t) => `opacity:${t}` };
}

/** Popover e card che compaiono: piccolo salto dall'alto. */
export function pop(_node: Element, { duration = 180 } = {}): TransitionConfig {
  if (reduced()) return { duration: 120, css: (t) => `opacity:${t}` };
  return { duration, easing: easeInk, css: (t, u) => `opacity:${t};transform:translateY(${-u * 6}px) scale(${1 - u * 0.02})` };
}
