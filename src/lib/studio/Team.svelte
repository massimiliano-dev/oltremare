<script lang="ts">
  // La squadra al lavoro: il senior che coordina, i sotto-agenti con i loro incarichi e azioni, i revisori,
  // e il filo degli scambi tra loro (incarichi, rapporti, verdetti).
  import { getContext } from "svelte";
  import Icon from "../Icon.svelte";
  import ToolGlyph, { toolKind } from "./ToolGlyph.svelte";
  import { markdown } from "../markdown";
  import type { Workspace as WS } from "../state/workspace.svelte";
  import { api, errorText, type AgentStep, type TeamAgent } from "../api";

  const ws = getContext<WS>("ws");

  const MODEL: Record<string, string> = { haiku: "Haiku", sonnet: "Sonnet", opus: "Opus", fable: "Fable", auto: "Auto" };
  const agents = $derived(ws.team?.agents ?? []);
  const task = $derived(ws.lastTask);
  const running = $derived(!!ws.dev.session?.running);
  let open = $state<Record<string, boolean>>({});

  // Il lavoro di un singolo agente, passo per passo, nella colonna di destra
  let selId = $state<string | null>(null);
  const sel = $derived(agents.find((a) => a.toolUseId === selId) ?? null);
  let log = $state<AgentStep[]>([]);
  let logError = $state("");
  let logBox: HTMLElement | undefined = $state();

  async function loadLog(a: TeamAgent) {
    if (!a.agentId) return;
    const giu = !logBox || logBox.scrollHeight - logBox.scrollTop - logBox.clientHeight < 80;
    try {
      const passi = await api.studioAgentLog(ws.path, a.agentId, ws.team?.session);
      if (selId !== a.toolUseId) return;
      log = passi;
      logError = "";
    } catch (e) {
      logError = errorText(e);
    }
    // Chi sta seguendo in fondo resta in fondo mentre l'agente lavora
    if (giu) requestAnimationFrame(() => logBox?.scrollTo({ top: logBox.scrollHeight }));
  }

  $effect(() => {
    const a = sel;
    if (!a?.agentId) return;
    loadLog(a);
    if (!a.running) return;
    const t = setInterval(() => loadLog(a), 2500);
    return () => clearInterval(t);
  });

  function segui(a: TeamAgent) {
    log = [];
    logError = "";
    selId = selId === a.toolUseId ? null : a.toolUseId;
  }

  /** Tipo di agente → etichetta e colore */
  function tipo(t: string): { label: string; h: number } {
    const n = t.toLowerCase();
    if (n.includes("backend")) return { label: "Backend", h: 165 };
    if (n.includes("frontend")) return { label: "Frontend", h: 305 };
    if (n === "general-purpose" || n === "claude") return { label: "Generico", h: 262 };
    if (n === "explore") return { label: "Esplora", h: 200 };
    if (n === "plan") return { label: "Piano", h: 285 };
    return { label: t.replace(/^oltremare-/, ""), h: 35 };
  }

  function durata(a: TeamAgent) {
    const ms = a.durationMs ?? ((a.endedAt ?? Date.now()) - a.startedAt);
    if (ms < 1000) return "";
    const s = Math.round(ms / 1000);
    return s < 60 ? `${s}s` : `${Math.floor(s / 60)}m ${s % 60}s`;
  }

  function ora(ms: number) {
    return new Date(ms).toLocaleTimeString("it-IT", { hour: "2-digit", minute: "2-digit" });
  }

  const senior = $derived.by(() => {
    if (!task) return { label: "In attesa", h: 262, live: false };
    switch (task.phase) {
      case "deciding":
        return { label: "Valuta la richiesta", h: 262, live: true };
      case "planning":
        return { label: "Prepara il piano", h: 285, live: true };
      case "plan-ready":
        return { label: "Piano pronto: aspetta te", h: 285, live: false };
      case "working":
        return { label: task.fixRounds ? "Corregge" : agents.some((a) => a.running) ? "Coordina la squadra" : "Al lavoro", h: 150, live: true };
      case "reviewing":
        return { label: "Aspetta la revisione", h: 80, live: true };
      case "review-ready":
        return { label: "Rilievi da decidere", h: 80, live: false };
      case "error":
        return { label: "Fermo", h: 25, live: false };
      default:
        return { label: "Fatto", h: 150, live: false };
    }
  });
  /** L'ultima azione del senior (non le deleghe) */
  const lastAction = $derived([...(ws.dev.session?.items ?? [])].reverse().find((i) => i.kind === "tool" && !["Agent", "Task"].includes(i.tool ?? "")));

  /** Il filo degli scambi, in ordine di tempo; i verdetti dei revisori in coda */
  const feed = $derived.by(() => {
    const out: { at: number; from: string; to: string; h: number; text: string; kind: "task" | "report" | "review" | "note" }[] = [];
    for (const a of agents) {
      const t = tipo(a.type);
      out.push({ at: a.startedAt, from: "Senior", to: t.label, h: t.h, text: a.description + (a.prompt && a.prompt !== a.description ? ` — ${a.prompt}` : ""), kind: "task" });
      if (a.endedAt) out.push({ at: a.endedAt, from: t.label, to: "Senior", h: t.h, text: a.error ? `Non riuscito: ${a.report}` : a.report || "(nessun rapporto)", kind: "report" });
    }
    out.sort((x, y) => x.at - y.at);
    if (task?.reviews?.length || task?.phase === "reviewing") {
      out.push({ at: 0, from: "Senior", to: "Revisori", h: 80, text: "Lavoro concluso: chiede la revisione delle modifiche", kind: "note" });
      for (const r of task?.reviews ?? []) {
        out.push({ at: 0, from: r.who, to: "Senior", h: r.who.startsWith("CTO") ? 150 : 35, text: `${r.review.esito === "approvato" ? "Approvato" : r.review.esito === "da correggere" ? "Da correggere" : "Giudizio"}${r.review.voto ? ` (${r.review.voto}/10)` : ""}${r.review.sintesi ? `: ${r.review.sintesi}` : ""}`, kind: "review" });
      }
      if (task?.fixRounds) out.push({ at: 0, from: "Senior", to: "Squadra", h: 150, text: "Rimette mano alle modifiche sui punti segnalati", kind: "note" });
    }
    return out;
  });
</script>

<div class="twrap">
<div class="team" class:detail={!!sel}>
  <section class="org" aria-label="Chi sta lavorando">
    <article class="node senior" class:live={senior.live} style="--h:{senior.h}">
      <header>
        <span class="av"><Icon name="spark" size={16} /></span>
        <div class="who">
          <strong>Senior</strong>
          <small>{task ? `${MODEL[task.model] ?? task.model}${task.profile ? ` · ${task.profile}` : ""}` : "Claude"}</small>
        </div>
        <span class="state"><span class="dot"></span>{senior.label}</span>
      </header>
      {#if task}<p class="brief">{task.text}</p>{/if}
      {#if lastAction && running && !agents.some((a) => a.running)}
        <div class="act"><ToolGlyph kind={toolKind(lastAction.tool ?? "")} size={18} /><span>{lastAction.title}</span></div>
      {/if}
    </article>

    {#if agents.length}
      <div class="rail" class:live={agents.some((a) => a.running)}></div>
      <div class="agents">
        {#each agents as a (a.toolUseId)}
          {@const t = tipo(a.type)}
          <article class="node agent" class:live={a.running} class:bad={a.error} class:sel={selId === a.toolUseId} style="--h:{t.h}">
            <header>
              <span class="av"><Icon name={a.type.includes("frontend") ? "palette" : a.type.includes("backend") ? "cpu" : "users"} size={15} /></span>
              <div class="who">
                <strong>{t.label}</strong>
                <small>{ora(a.startedAt)}{durata(a) ? ` · ${durata(a)}` : ""}{a.toolUses ? ` · ${a.toolUses} ${a.toolUses === 1 ? "azione" : "azioni"}` : ""}</small>
              </div>
              <span class="state"><span class="dot"></span>{a.running ? "al lavoro" : a.error ? "non riuscito" : "fatto"}</span>
            </header>
            <p class="brief">{a.description}</p>
            {#if a.steps.length}
              <ol class="steps">
                {#each a.steps.slice(-3) as s, i (i)}
                  <li><ToolGlyph kind={toolKind(s.tool)} size={16} /><span>{s.title || s.tool}</span></li>
                {/each}
              </ol>
            {/if}
            {#if a.running && a.progress}<p class="prog">{a.progress}</p>{/if}
            {#if a.agentId}
              <button class="see" class:on={selId === a.toolUseId} onclick={() => segui(a)} aria-pressed={selId === a.toolUseId}>
                <Icon name="rows" size={12} /> {selId === a.toolUseId ? "Chiudi il dettaglio" : a.running ? "Segui il lavoro" : "Vedi cosa ha fatto"}
              </button>
            {/if}
            {#if a.report}
              <button class="rep" onclick={() => (open[a.toolUseId] = !open[a.toolUseId])} aria-expanded={!!open[a.toolUseId]}>
                <Icon name={open[a.toolUseId] ? "down" : "chevron"} size={11} /> Rapporto
              </button>
              {#if open[a.toolUseId]}<div class="md rtext">{@html markdown(a.report)}</div>{/if}
            {/if}
          </article>
        {/each}
      </div>
    {/if}

    {#if task?.reviews?.length || task?.phase === "reviewing"}
      <div class="rail" class:live={task.phase === "reviewing"}></div>
      <div class="agents">
        {#each task.reviews ?? [] as r, i (i)}
          <article class="node agent" style="--h:{r.who.startsWith('CTO') ? 150 : 35}">
            <header>
              <span class="av"><Icon name={r.who.startsWith("CTO") ? "shield" : "users"} size={15} /></span>
              <div class="who"><strong>{r.who}</strong><small>revisione in sola lettura</small></div>
              <span class="state"><span class="dot"></span>{r.review.esito === "approvato" ? "approva" : r.review.esito === "da correggere" ? "chiede correzioni" : "giudizio"}</span>
            </header>
            {#if r.review.sintesi}<p class="brief">{r.review.sintesi}</p>{/if}
          </article>
        {/each}
        {#if task.phase === "reviewing"}
          <article class="node agent live" style="--h:{task.reviews?.length ? 35 : 150}">
            <header>
              <span class="av"><Icon name={task.reviews?.length ? "users" : "shield"} size={15} /></span>
              <div class="who"><strong>{task.reviews?.length ? "Secondo revisore" : "CTO"}</strong><small>legge le modifiche ed esegue i test</small></div>
              <span class="state"><span class="dot"></span>al lavoro</span>
            </header>
          </article>
        {/if}
      </div>
    {/if}

    {#if !agents.length && !task}
      <div class="empty">
        <p>Qui vedi la squadra mentre lavora: il senior che coordina, i sotto-agenti con i loro incarichi e i revisori. Chiedi qualcosa all'orchestratore per cominciare.</p>
        {#if ws.agentDefs.length}
          <div class="defs">
            <strong>Agenti pronti</strong>
            {#each ws.agentDefs as d (d.name)}
              {@const t = tipo(d.name)}
              <div class="def" style="--h:{t.h}"><span class="chip">{d.oltremare ? t.label : d.name}</span><span>{d.description}</span></div>
            {/each}
            <small>Il senior può crearne altri al volo con un incarico su misura.</small>
          </div>
        {/if}
      </div>
    {/if}
  </section>

  {#if sel}
    {@const t = tipo(sel.type)}
    <aside class="feed log" aria-label="Il lavoro di {t.label}" style="--h:{t.h}" bind:this={logBox}>
      <div class="lh">
        <h3>{t.label}: {sel.description}</h3>
        <button class="icon-btn" onclick={() => (selId = null)} title="Torna agli scambi" aria-label="Torna agli scambi"><Icon name="x" size={13} /></button>
      </div>
      <p class="meta">{sel.running ? "Al lavoro" : sel.error ? "Non riuscito" : "Concluso"} · {sel.toolUses} {sel.toolUses === 1 ? "azione" : "azioni"}{durata(sel) ? ` · ${durata(sel)}` : ""}</p>
      {#if logError}<p class="none">Lettura non riuscita: {logError}</p>
      {:else if !log.length}<p class="none">Leggo il suo lavoro…</p>{/if}
      <ol class="steps-log">
        {#each log as p, i (i)}
          {#if p.kind === "prompt"}
            <li class="l-prompt">
              <details>
                <summary>Incarico dal senior</summary>
                <div class="md">{@html markdown(p.text ?? "")}</div>
              </details>
            </li>
          {:else if p.kind === "text"}
            <li class="l-text"><div class="md">{@html markdown(p.text ?? "")}</div><time>{ora(p.at)}</time></li>
          {:else}
            <li class="l-tool" class:err={p.error} class:wait={p.result === undefined && sel.running}>
              <details>
                <summary><ToolGlyph kind={toolKind(p.tool ?? "")} size={16} /><span class="tt">{p.title || p.tool}</span><span class="tn">{p.tool}</span></summary>
                {#if p.input}<pre>{p.input}</pre>{/if}
                {#if p.result !== undefined}<pre class="res">{p.result || "(nessun output)"}</pre>{:else}<p class="none">{sel.running ? "In corso…" : "Nessun esito"}</p>{/if}
              </details>
            </li>
          {/if}
        {/each}
      </ol>
      {#if sel.report}
        <div class="md rtext final">{@html markdown(sel.report)}</div>
      {/if}
    </aside>
  {:else}
  <aside class="feed" aria-label="Scambi tra gli agenti">
    <h3>Scambi</h3>
    <p class="meta">
      {#if ws.teamError}Lettura non riuscita: {ws.teamError}
      {:else if ws.team}Sessione {ws.team.session ? ws.team.session.slice(0, 8) : "—"} · {ws.team.agents.length} {ws.team.agents.length === 1 ? "sotto-agente" : "sotto-agenti"} · letta alle {ws.teamAt ? new Date(ws.teamAt).toLocaleTimeString("it-IT") : "—"}
      {:else}In attesa della prima lettura…{/if}
    </p>
    {#if !feed.length}
      <p class="none">Nessuno scambio ancora.</p>
    {:else}
      <ol>
        {#each feed as f, i (i)}
          <li class="f-{f.kind}" style="--h:{f.h}">
            <div class="fh"><span class="from">{f.from}</span><Icon name="chevron" size={10} /><span class="to">{f.to}</span>{#if f.at}<time>{ora(f.at)}</time>{/if}</div>
            <p>{f.text}</p>
          </li>
        {/each}
      </ol>
    {/if}
  </aside>
  {/if}
</div>
</div>

<style>
  .twrap {
    flex: 1;
    min-height: 0;
    min-width: 0;
    display: flex;
    container-type: inline-size;
  }
  .team {
    flex: 1;
    height: 100%;
    min-height: 0;
    min-width: 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr) 300px;
  }
  /* Nella colonna centrale dello Studio lo spazio è poco: scambi sotto, non di fianco */
  @container (max-width: 820px) {
    .team {
      grid-template-columns: 1fr;
      grid-template-rows: minmax(0, 1fr) minmax(150px, 40%);
    }
    .team.detail {
      grid-template-rows: minmax(0, 40%) minmax(0, 1fr);
    }
    .feed {
      border-left: 0 !important;
      border-top: 1px solid var(--rule-soft);
    }
  }
  .org {
    overflow-y: auto;
    min-width: 0;
    padding: 20px 22px 28px;
    display: grid;
    align-content: start;
    justify-items: center;
    gap: 0;
    background: radial-gradient(600px 300px at 50% 0%, color-mix(in oklch, var(--accent) 6%, transparent), transparent 70%);
  }
  .node {
    width: 100%;
    min-width: 0;
    display: grid;
    gap: 10px;
    padding: 14px 16px;
    border-radius: 14px;
    background: var(--paper);
    box-shadow: inset 0 0 0 1px var(--rule-soft), 0 1px 2px oklch(0.3 0.05 var(--tint-h) / 0.05);
    transition: box-shadow 0.3s;
  }
  .node.live {
    box-shadow: inset 0 0 0 1.5px oklch(0.65 0.14 var(--h)), 0 0 0 4px oklch(0.65 0.14 var(--h) / 0.1);
  }
  .node.bad {
    box-shadow: inset 0 0 0 1.5px var(--danger);
  }
  .senior {
    max-width: 640px;
  }
  .node header {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px 10px;
    min-width: 0;
  }
  .av {
    flex: none;
    width: 30px;
    height: 30px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    color: oklch(0.45 0.14 var(--h));
    background: oklch(0.75 0.12 var(--h) / 0.22);
  }
  :global([data-theme="dark"]) .av {
    color: oklch(0.85 0.11 var(--h));
  }
  .who {
    flex: 1;
    min-width: 0;
    display: grid;
  }
  .who strong {
    font-size: var(--t-sm);
    font-weight: 650;
    overflow-wrap: anywhere;
  }
  .who small {
    font-size: var(--t-xs);
    color: var(--ink-3);
    overflow-wrap: anywhere;
  }
  .state {
    flex: none;
    margin-left: auto;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 2px 9px;
    border-radius: 999px;
    font-size: 0.68rem;
    font-weight: 650;
    color: oklch(0.45 0.13 var(--h));
    background: oklch(0.75 0.12 var(--h) / 0.18);
  }
  :global([data-theme="dark"]) .state {
    color: oklch(0.85 0.11 var(--h));
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: oklch(0.62 0.16 var(--h));
  }
  .live .dot {
    animation: beat 1.1s ease-in-out infinite;
  }
  @keyframes beat {
    50% {
      transform: scale(0.5);
      opacity: 0.4;
    }
  }
  .brief {
    margin: 0;
    font-size: var(--t-sm);
    line-height: 1.5;
    color: var(--ink-2);
    overflow-wrap: anywhere;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .act,
  .steps li {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    font-size: var(--t-xs);
    color: var(--ink-3);
    font-family: ui-monospace, "SF Mono", Menlo, monospace;
  }
  .act span,
  .steps li span {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .steps {
    list-style: none;
    margin: 0;
    padding: 10px 12px;
    display: grid;
    gap: 6px;
    border-radius: 9px;
    background: var(--paper-2);
  }
  .prog {
    margin: 0;
    font-size: var(--t-xs);
    color: var(--ink-3);
    font-style: italic;
    overflow-wrap: anywhere;
  }
  .rep {
    justify-self: start;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: var(--t-xs);
    font-weight: 600;
    color: var(--accent);
  }
  .rtext {
    font-size: var(--t-xs);
    padding: 8px 10px;
    border-radius: 8px;
    background: var(--paper-2);
    max-height: 260px;
    overflow: auto;
  }
  .rail {
    width: 2px;
    height: 32px;
    background: var(--rule);
    border-radius: 2px;
  }
  .rail.live {
    background: linear-gradient(var(--accent) 0 40%, transparent 40% 60%, var(--accent) 60%);
    background-size: 100% 12px;
    animation: flow 0.9s linear infinite;
  }
  @keyframes flow {
    to {
      background-position: 0 12px;
    }
  }
  .agents {
    width: 100%;
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 260px), 1fr));
    gap: 12px;
  }
  .empty {
    max-width: 520px;
    margin-top: 26px;
    display: grid;
    gap: 14px;
    text-align: center;
    color: var(--ink-3);
    font-size: var(--t-sm);
    line-height: 1.5;
  }
  .empty p {
    margin: 0;
  }
  .defs {
    display: grid;
    gap: 8px;
    padding: 12px 14px;
    border-radius: 12px;
    text-align: left;
    background: var(--paper);
    box-shadow: inset 0 0 0 1px var(--rule-soft);
  }
  .defs strong {
    font-size: var(--t-xs);
    color: var(--ink-2);
  }
  .def {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    font-size: var(--t-xs);
    color: var(--ink-2);
  }
  .chip {
    flex: none;
    padding: 1px 8px;
    border-radius: 999px;
    font-weight: 700;
    color: oklch(0.45 0.13 var(--h));
    background: oklch(0.75 0.12 var(--h) / 0.2);
  }
  :global([data-theme="dark"]) .chip {
    color: oklch(0.85 0.11 var(--h));
  }
  .feed {
    min-width: 0;
    overflow-y: auto;
    padding: 16px 16px 22px;
    border-left: 1px solid var(--rule-soft);
    background: var(--paper-2);
  }
  .feed h3 {
    margin: 0 0 10px;
    font-size: var(--t-xs);
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--ink-2);
  }
  .none,
  .meta {
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
  .meta {
    margin: -6px 0 10px;
    font-variant-numeric: tabular-nums;
  }
  .feed ol {
    list-style: none;
    margin: 0;
    padding: 0 0 0 12px;
    display: grid;
    gap: 10px;
    border-left: 1.5px solid var(--rule);
  }
  .feed li {
    position: relative;
    display: grid;
    gap: 4px;
    min-width: 0;
    padding: 9px 11px;
    border-radius: 9px;
    background: var(--paper);
    font-size: var(--t-xs);
  }
  .feed li::before {
    content: "";
    position: absolute;
    left: -17.5px;
    top: 13px;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: oklch(0.65 0.14 var(--h));
  }
  .fh {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 4px;
    font-weight: 650;
    color: oklch(0.45 0.13 var(--h));
  }
  :global([data-theme="dark"]) .fh {
    color: oklch(0.85 0.11 var(--h));
  }
  .fh time {
    margin-left: auto;
    font-weight: 500;
    color: var(--ink-3);
  }
  .feed p {
    margin: 0;
    color: var(--ink-2);
    line-height: 1.5;
    overflow-wrap: anywhere;
    display: -webkit-box;
    -webkit-line-clamp: 4;
    line-clamp: 4;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  @container (min-width: 821px) {
    .team.detail {
      grid-template-columns: minmax(0, 1fr) minmax(320px, 44%);
    }
  }
  .node.sel {
    box-shadow: inset 0 0 0 1.5px oklch(0.6 0.15 var(--h)), 0 0 0 4px oklch(0.65 0.14 var(--h) / 0.14);
  }
  .see {
    justify-self: start;
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 3px 9px;
    border-radius: 999px;
    font-size: var(--t-xs);
    font-weight: 600;
    color: oklch(0.45 0.13 var(--h));
    background: oklch(0.75 0.12 var(--h) / 0.14);
  }
  .see:hover,
  .see.on {
    background: oklch(0.75 0.12 var(--h) / 0.26);
  }
  :global([data-theme="dark"]) .see {
    color: oklch(0.85 0.11 var(--h));
  }
  .lh {
    display: flex;
    align-items: flex-start;
    gap: 8px;
  }
  .lh h3 {
    flex: 1;
    min-width: 0;
    text-transform: none;
    letter-spacing: 0;
    font-size: var(--t-sm);
    line-height: 1.35;
  }
  .steps-log {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    gap: 6px;
  }
  .steps-log li {
    min-width: 0;
    border-radius: 9px;
    background: var(--paper);
    font-size: var(--t-xs);
  }
  .steps-log summary {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 7px 10px;
    cursor: pointer;
    list-style: none;
    font-weight: 600;
    color: var(--ink);
  }
  .steps-log summary::-webkit-details-marker {
    display: none;
  }
  .tt {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tn {
    flex: none;
    font-weight: 500;
    color: var(--ink-3);
  }
  .l-text {
    display: grid;
    gap: 2px;
    padding: 8px 10px;
    color: var(--ink-2);
    box-shadow: inset 2px 0 0 oklch(0.65 0.14 var(--h));
  }
  .l-text time {
    justify-self: end;
    color: var(--ink-3);
    font-variant-numeric: tabular-nums;
  }
  .l-prompt .md,
  .l-tool pre,
  .l-tool .none {
    margin: 0 10px 8px;
  }
  .l-prompt .md {
    max-height: 320px;
    overflow: auto;
    color: var(--ink-2);
  }
  .l-tool pre {
    max-height: 240px;
    overflow: auto;
    padding: 7px 9px;
    border-radius: 7px;
    background: var(--paper-2);
    font: 0.7rem/1.45 ui-monospace, "SF Mono", Menlo, monospace;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    color: var(--ink-2);
  }
  .l-tool pre.res {
    color: var(--ink);
  }
  .l-tool.err {
    box-shadow: inset 2px 0 0 var(--danger);
  }
  .l-tool.err pre.res {
    color: var(--danger);
  }
  .l-tool.wait summary {
    animation: attesa 1.4s ease-in-out infinite;
  }
  @keyframes attesa {
    50% {
      opacity: 0.55;
    }
  }
  .final {
    margin-top: 10px;
    max-height: none;
    box-shadow: inset 2px 0 0 var(--ok);
  }
  .f-report {
    box-shadow: inset 2px 0 0 oklch(0.65 0.14 var(--h));
  }
  @media (prefers-reduced-motion: reduce) {
    .live .dot,
    .l-tool.wait summary,
    .rail.live {
      animation: none;
    }
  }
</style>
