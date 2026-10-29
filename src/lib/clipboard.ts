// Copia negli appunti: prima il browser, poi il comando dell'app (il webview può rifiutare senza gesto dell'utente).
import { invoke } from "@tauri-apps/api/core";
import { ui } from "./state/ui.svelte";

export async function copia(testo: string, cosa = "Messaggio") {
  try {
    await navigator.clipboard.writeText(testo);
  } catch {
    try {
      await invoke("clipboard_write", { text: testo });
    } catch {
      ui.toast("Non riesco a copiare", "error");
      return;
    }
  }
  ui.toast(`${cosa} copiato`, "info", undefined, 1800);
}
