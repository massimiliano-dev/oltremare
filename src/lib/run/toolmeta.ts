// Icone ed etichette per strumenti e categorie.
import type { Risk, ToolCategory } from "../api";

export const CATEGORY: Record<string, { label: string; icon: string }> = {
  fs: { label: "File", icon: "folder" },
  shell: { label: "Terminale", icon: "terminal" },
  apps: { label: "App e link", icon: "external" },
  clipboard: { label: "Appunti", icon: "copy" },
  computer: { label: "Schermo", icon: "target" },
  browser: { label: "Browser", icon: "globe" },
  mail: { label: "Posta", icon: "mail" },
  calendar: { label: "Calendario", icon: "calendar" },
  tasks: { label: "Attività", icon: "check" },
  notifications: { label: "Notifiche", icon: "bell" },
  dev: { label: "Sviluppo", icon: "terminal" },
  media: { label: "Contenuti", icon: "image" },
  web: { label: "Ricerca web", icon: "search" },
  other: { label: "Altro", icon: "spark" },
};

/** Etichetta e icona di una categoria; i server MCP prendono il loro nome. */
export function categoryMeta(cat: string, serverName?: (id: string) => string): { label: string; icon: string } {
  if (cat.startsWith("mcp:")) return { label: serverName ? serverName(cat.slice(4)) : "Server esterno", icon: "layers" };
  return CATEGORY[cat] ?? { label: cat, icon: "spark" };
}

export const RISK: Record<Risk, string> = { read: "lettura", write: "modifica", danger: "rischio alto" };

export function toolIcon(category: string | undefined, tool?: string): string {
  if (tool === "cestina_file") return "trash";
  if (tool === "scrivi_file") return "pen";
  if (tool === "leggi_file") return "draft";
  if (tool === "cerca_file") return "search";
  if (tool === "schermo_screenshot" || tool === "schermo_zoom") return "image";
  if (tool === "schermo_scrivi" || tool === "schermo_tasti") return "command";
  if (tool === "attendi") return "clock";
  if (category?.startsWith("mcp:")) return "layers";
  if (tool === "sviluppa") return "terminal";
  if (tool === "crea_pdf" || tool === "crea_presentazione") return "draft";
  if (tool === "crea_video") return "video";
  if (tool === "cerca_web") return "search";
  if (tool === "leggi_pagina") return "globe";
  if (tool === "apri_in_editor") return "pen";
  return CATEGORY[(category as ToolCategory) ?? "other"]?.icon ?? "spark";
}
