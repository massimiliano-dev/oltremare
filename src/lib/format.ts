const timeFmt = new Intl.DateTimeFormat("it-IT", { hour: "2-digit", minute: "2-digit" });
const dayFmt = new Intl.DateTimeFormat("it-IT", { weekday: "short" });
const shortFmt = new Intl.DateTimeFormat("it-IT", { day: "numeric", month: "short" });
const yearFmt = new Intl.DateTimeFormat("it-IT", { day: "numeric", month: "short", year: "numeric" });

function startOfDay(d: Date) {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
}

/** Data compatta (ms): ora se oggi, giorno se in settimana, altrimenti data. */
export function listDate(ms: number): string {
  if (!ms) return "";
  const d = new Date(ms);
  const today = startOfDay(new Date());
  const day = startOfDay(d);
  if (day === today) return timeFmt.format(d);
  if (today - day < 6 * 86400000) return dayFmt.format(d).replace(".", "");
  if (d.getFullYear() === new Date().getFullYear()) return shortFmt.format(d).replace(".", "");
  return yearFmt.format(d);
}

export function time(ms: number): string {
  return timeFmt.format(new Date(ms));
}

/** "4,2 s", "1 min 12 s" */
export function duration(ms: number): string {
  if (ms < 1000) return `${Math.max(0, Math.round(ms))} ms`;
  const s = ms / 1000;
  if (s < 60) return `${s.toFixed(s < 10 ? 1 : 0).replace(".", ",")} s`;
  const m = Math.floor(s / 60);
  return `${m} min ${Math.round(s % 60)} s`;
}

export function tokens(n: number): string {
  if (n < 1000) return String(n);
  if (n < 1_000_000) return `${(n / 1000).toFixed(n < 10_000 ? 1 : 0).replace(".", ",")}k`;
  return `${(n / 1_000_000).toFixed(1).replace(".", ",")}M`;
}

export function money(usd: number): string {
  if (usd < 0.01) return "< 0,01 $";
  return `${usd.toFixed(usd < 1 ? 3 : 2).replace(".", ",")} $`;
}

export function fileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  if (bytes < 1024 ** 3) return `${(bytes / 1024 / 1024).toFixed(1).replace(".", ",")} MB`;
  return `${(bytes / 1024 ** 3).toFixed(1).replace(".", ",")} GB`;
}

export function escapeHtml(s: string): string {
  return s.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]!);
}

/** Saluto in base all'ora. */
export function greeting(name: string): string {
  const h = new Date().getHours();
  const base = h < 5 ? "Buonanotte" : h < 13 ? "Buongiorno" : h < 18 ? "Buon pomeriggio" : "Buonasera";
  return name.trim() ? `${base}, ${name.trim().split(" ")[0]}` : base;
}
