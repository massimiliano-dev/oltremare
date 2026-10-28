// File locali (screenshot) mostrati nella finestra tramite il protocollo asset di Tauri.
import { convertFileSrc } from "@tauri-apps/api/core";

export function assetUrl(path: string | null | undefined): string {
  if (!path) return "";
  if (path.startsWith("data:") || path.startsWith("http")) return path;
  try {
    return convertFileSrc(path);
  } catch {
    return path;
  }
}
