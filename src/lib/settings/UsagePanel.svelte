<script lang="ts">
  // Utilizzo dei provider negli ultimi 30 giorni: totali, andamento giornaliero, agenti e modelli.
  // Con l'abbonamento: stato della finestra di utilizzo del piano e valore equivalente.
  import { onMount } from "svelte";
  import Icon from "../Icon.svelte";
  import { api, type Provider, type UsageProvider, type UsageStats } from "../api";
  import { providerLabel } from "../state/settings.svelte";
  import { agents } from "../state/agents.svelte";
  import { tokens, money } from "../format";

  let stats = $state<UsageStats | null>(null);
  let hover = $state<{ p: string; i: number } | null>(null);
  onMount(async () => {
    stats = await api.usageStats().catch(() => null);
  });

  const ORDER: Provider[] = ["claude-code", "anthropic", "openai", "local"];
  const list = $derived(ORDER.filter((p) => stats?.providers[p]).map((p) => [p, stats!.providers[p]!] as const));
  const paid = (p: Provider) => p === "anthropic" || p === "openai";
  const sub = $derived(stats?.subscription ?? null);

  function giorno(i: number) {
    const d = new Date((stats?.since ?? 0) + i * 86_400_000);
    return d.toLocaleDateString("it-IT", { weekday: "short", day: "numeric", month: "short" });
  }

  /** La grandezza del grafico: costo per i provider a pagamento, token per gli altri. */
  function serie(p: Provider, u: UsageProvider) {
    return u.daily.map((d) => (paid(p) ? d.cost : d.tokens));
  }

  function fmt(p: Provider, v: number) {
    return paid(p) ? money(v) : `${tokens(v)} token`;
  }

  function stato(s: string | undefined) {
    if (s === "allowed") return { label: "Disponibile", icon: "check", cls: "ok" };
    if (s === "allowed_warning") return { label: "Vicino al limite", icon: "alert", cls: "warn" };
    if (s === "rejected") return { label: "Limite raggiunto", icon: "stop", cls: "bad" };
    return { label: "Sconosciuto", icon: "info", cls: "" };
  }

  function ora(sec: number | undefined) {
    if (!sec) return "";
    const d = new Date(sec * 1000);
    const oggi = new Date().toDateString() === d.toDateString();
    return oggi ? `alle ${d.toLocaleTimeString("it-IT", { hour: "2-digit", minute: "2-digit" })}` : d.toLocaleString("it-IT", { weekday: "short", hour: "2-digit", minute: "2-digit" });
  }

  const W = 420;
  const H = 70;
  function geometry(vals: number[]) {
    const max = Math.max(...vals, 0);
    const n = vals.length;
    const gap = 2;
    const bw = (W - gap * (n - 1)) / n;
    return { max, bars: vals.map((v, i) => ({ x: i * (bw + gap), w: bw, h: max ? Math.max(v > 0 ? 2 : 0, (v / max) * (H - 4)) : 0 })) };
  }

  function bar(x: number, w: number, h: number) {
    // Estremo arrotondato verso l'alto, base dritta sull'asse
    if (h <= 0) return "";
    const r = Math.min(4, w / 2, h);
    const y = H - h;
    return `M${x},${H} V${y + r} Q${x},${y} ${x + r},${y} H${x + w - r} Q${x + w},${y} ${x + w},${y + r} V${H} Z`;
  }

  function top(rec: Record<string, { runs?: number; input?: number; output?: number; cost?: number }>, n = 4) {
    return Object.entries(rec)
      .sort((a, b) => (b[1].runs ?? 0) - (a[1].runs ?? 0))
      .slice(0, n);
  }
</script>

{#if !stats}
  <div class="skeleton" style="height:120px"></div>
{:else if !list.length}
  <p class="empty">Ancora nessun utilizzo da mostrare: le statistiche arrivano con le prime richieste agli agenti.</p>
{:else}
  {#each list as [p, u] (p)}
    {@const vals = serie(p, u)}
    {@const g = geometry(vals)}
    <div class="prov">
      <div class="phead">
        <strong>{providerLabel(p)}</strong>
        <small>{p === "claude-code" ? "incluso nel piano" : paid(p) ? "costo stimato dai token" : "sul tuo computer, gratis"}</small>
      </div>

      {#if p === "claude-code" && sub}
        {@const st = stato(sub.status)}
        <div class="limit {st.cls}">
          <Icon name={st.icon} size={14} />
          <span><strong>{st.label}</strong> · finestra di {sub.rateLimitType === "five_hour" ? "5 ore" : sub.rateLimitType === "seven_day" ? "7 giorni" : "utilizzo"}{sub.resetsAt ? `, si azzera ${ora(sub.resetsAt)}` : ""}</span>
          {#if sub.overageStatus}<small>Uso extra: {sub.isUsingOverage ? "in corso" : sub.overageStatus === "rejected" ? "non attivo" : sub.overageStatus}</small>{/if}
        </div>
      {/if}

      <div class="tiles">
        {#each [["Oggi", u.today], ["7 giorni", u.week], ["30 giorni", u.month]] as const as [lab, t] (lab)}
          <div class="tile">
            <span class="tl">{lab}</span>
            <span class="tv">{paid(p) ? money(t.cost ?? 0) : `${t.runs ?? 0} richieste`}</span>
            <span class="ts">
              {#if paid(p)}{t.runs ?? 0} richieste · {/if}{tokens((t.input ?? 0) + (t.output ?? 0))} token{#if (t.searches ?? 0) > 0} · {t.searches} ricerche{/if}
              {#if p === "claude-code" && (t.value ?? 0) > 0}<br />valore API {money(t.value ?? 0)}{/if}
            </span>
          </div>
        {/each}
      </div>

      <div class="chart">
        <div class="clab">
          <span>{paid(p) ? "Costo al giorno" : "Token al giorno"}, ultimi 30 giorni</span>
          {#if g.max > 0}<span class="max">max {fmt(p, g.max)}</span>{/if}
        </div>
        <svg viewBox="0 0 {W} {H + 1}" preserveAspectRatio="none" role="img" aria-label="{paid(p) ? 'Costo' : 'Token'} al giorno negli ultimi 30 giorni">
          <line x1="0" x2={W} y1={H + 0.5} y2={H + 0.5} class="axis" />
          {#each g.bars as b, i (i)}
            <path d={bar(b.x, b.w, b.h)} class="bar" class:dim={hover && hover.p === p && hover.i !== i} />
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <rect x={b.x - 1} y="0" width={b.w + 2} height={H} class="hit" onmouseenter={() => (hover = { p, i })} onmouseleave={() => (hover = null)} />
          {/each}
        </svg>
        <div class="xl"><span>{giorno(0)}</span><span>oggi</span></div>
        {#if hover && hover.p === p}
          {@const d = u.daily[hover.i]}
          {@const pos = hover.i / (u.daily.length - 1)}
          <div class="tip" style="left: {pos * 100}%; --tx: {pos > 0.7 ? '-100%' : pos < 0.3 ? '0%' : '-50%'}">
            <strong>{giorno(hover.i)}</strong>
            <span>{d.runs} richieste · {tokens(d.tokens)} token{#if paid(p)} · {money(d.cost)}{/if}{#if p === "claude-code" && d.value > 0} · valore {money(d.value)}{/if}</span>
          </div>
        {/if}
      </div>

      {#if Object.keys(u.agents).length || Object.keys(u.models).length}
        <div class="split">
          <div>
            <span class="sh">Agenti</span>
            {#each top(u.agents) as [id, t] (id)}
              <div class="li"><span>{agents.byId(id)?.name ?? "Agente eliminato"}</span><em>{t.runs} · {paid(p) ? money(t.cost ?? 0) : `${tokens((t.input ?? 0) + (t.output ?? 0))}`}</em></div>
            {/each}
          </div>
          <div>
            <span class="sh">Modelli</span>
            {#each top(u.models) as [m, t] (m)}
              <div class="li"><span class="mono">{m.replace(/^claude-/, "")}</span><em>{t.runs} · {paid(p) ? money(t.cost ?? 0) : `${tokens((t.input ?? 0) + (t.output ?? 0))}`}</em></div>
            {/each}
          </div>
        </div>
      {/if}
    </div>
  {/each}
  <small class="help">I costi sono stime dai token e dai listini; quelli reali stanno nelle console di Anthropic e OpenAI. Con l'abbonamento il «valore API» è quanto costerebbero le stesse richieste con la chiave.</small>
{/if}

<style>
  .empty {
    font-size: var(--t-sm);
    color: var(--ink-3);
  }
  .prov {
    display: grid;
    gap: 10px;
    padding: 12px 14px 14px;
    border-radius: var(--r-md);
    box-shadow: inset 0 0 0 1px var(--rule);
  }
  .phead {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }
  .phead strong {
    font-weight: 650;
  }
  .phead small {
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
  .limit {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px 8px;
    padding: 7px 10px;
    border-radius: var(--r-sm);
    background: var(--paper-2);
    font-size: var(--t-sm);
    color: var(--ink-2);
  }
  .limit.ok :global(svg) {
    color: var(--ok);
  }
  .limit.warn :global(svg) {
    color: var(--warn);
  }
  .limit.bad :global(svg) {
    color: var(--danger);
  }
  .limit small {
    margin-left: auto;
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
  .tiles {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 8px;
  }
  .tile {
    display: grid;
    gap: 2px;
    padding: 8px 10px;
    border-radius: var(--r-sm);
    background: var(--paper-2);
  }
  .tl {
    font-size: 0.68rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--ink-3);
  }
  .tv {
    font-size: 1.12rem;
    font-weight: 650;
    color: var(--ink);
    font-variant-numeric: tabular-nums;
  }
  .ts {
    font-size: 0.7rem;
    color: var(--ink-3);
    line-height: 1.4;
  }
  .chart {
    position: relative;
    display: grid;
    gap: 4px;
  }
  .clab {
    display: flex;
    justify-content: space-between;
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
  svg {
    width: 100%;
    height: 72px;
    display: block;
    overflow: visible;
  }
  .axis {
    stroke: var(--rule);
    stroke-width: 1;
    vector-effect: non-scaling-stroke;
  }
  .bar {
    fill: var(--chart-1);
    transition: opacity 0.12s;
  }
  .bar.dim {
    opacity: 0.35;
  }
  .hit {
    fill: transparent;
    cursor: default;
  }
  .xl {
    display: flex;
    justify-content: space-between;
    font-size: 0.68rem;
    color: var(--ink-3);
  }
  .tip {
    position: absolute;
    top: -6px;
    transform: translate(var(--tx, -50%), -100%);
    display: grid;
    gap: 1px;
    padding: 6px 9px;
    border-radius: var(--r-sm);
    background: var(--paper);
    box-shadow: var(--shadow-sheet);
    font-size: var(--t-xs);
    color: var(--ink-2);
    white-space: nowrap;
    pointer-events: none;
    z-index: 5;
  }
  .tip strong {
    color: var(--ink);
  }
  .split {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 14px;
  }
  .sh {
    display: block;
    margin-bottom: 4px;
    font-size: 0.68rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--ink-3);
  }
  .li {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    font-size: var(--t-sm);
    padding: 2px 0;
  }
  .li span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .li em {
    font-style: normal;
    color: var(--ink-3);
    font-variant-numeric: tabular-nums;
    flex: none;
  }
  .mono {
    font-family: var(--font-mono, ui-monospace, monospace);
    font-size: 0.92em;
  }
</style>
