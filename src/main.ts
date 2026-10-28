import "@fontsource-variable/instrument-sans/wght.css";
import "@fontsource/instrument-serif/latin-400.css";
import "@fontsource/instrument-serif/latin-400-italic.css";
import "./app.css";
import { mount } from "svelte";
import App from "./App.svelte";
import CompanionApp from "./CompanionApp.svelte";
import PaletteApp from "./PaletteApp.svelte";
import IslandApp from "./IslandApp.svelte";
import StudioApp from "./StudioApp.svelte";
import { theme } from "./lib/state/theme.svelte";

// Fuori da Tauri, in sviluppo, si lavora sulla grafica con dati finti
if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window && (window as any).__TAURI_INTERNALS__?.metadata)) {
  const { installPreview } = await import("./dev/preview");
  installPreview();
}

// Il tema si applica prima del primo disegno: niente lampi di bianco di notte
theme.boot();

// Stesso pacchetto per tutte le finestre: la vista si sceglie dall'indirizzo
const view = new URLSearchParams(location.search).get("view");
const Root = view === "avatar" ? CompanionApp : view === "palette" ? PaletteApp : view === "island" ? IslandApp : view === "studio" ? StudioApp : App;
const app = mount(Root, { target: document.getElementById("app")! });

export default app;
