<script lang="ts">
  import Icon from "../Icon.svelte";
  import Composer from "../chat/Composer.svelte";
  import AgentDisc from "../agents/AgentDisc.svelte";
  import Avatar from "../avatar/Avatar.svelte";
  import { markdown } from "../markdown";
  import { duration, greeting, time } from "../format";
  import { agents } from "../state/agents.svelte";
  import { runs } from "../state/runs.svelte";
  import { settings, providerLabel, modelLabel } from "../state/settings.svelte";
  import { avatar } from "../state/avatar.svelte";
  import { ui } from "../state/ui.svelte";
  import type { Run } from "../api";
  import { fileIcon, fileExt } from "../chat/files";
  import ToolCard from "../run/ToolCard.svelte";
  import ApprovalCard from "../run/ApprovalCard.svelte";
  import { approvals } from "../state/approvals.svelte";
  import { fileSize } from "../format";
  import { assetUrl } from "../assets";

  let scroller: HTMLDivElement | undefined = $state();
  let composer: ReturnType<typeof Composer> | undefined = $state();
  let pinned = true;
  let openThinking = $state<Record<string, boolean>>({});

  const SUGGESTIONS = [
    { icon: "calendar", text: "Aiutami a organizzare la giornata di domani" },
    { icon: "mail", text: "Scrivi una mail cordiale per rimandare una riunione" },
    { icon: "bulb", text: "Spiegami in parole semplici come funziona un mutuo a tasso variabile" },
    { icon: "all", text: "Fammi una lista della spesa per una cena per sei" },
  ];

  const agent = $derived(agents.selected);
  const showThinking = $derived(settings.s?.showThinking ?? true);

  function onScroll() {
    if (!scroller) return;
    pinned = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight < 60;
  }

  // Resta in fondo mentre arriva il testo, se l'utente non è risalito a leggere
  $effect(() => {
    for (const r of runs.current) for (const s of r.steps) void s.text;
    void runs.current.length;
    if (pinned && scroller) requestAnimationFrame(() => scroller && (scroller.scrollTop = scroller.scrollHeight));
  });

  $effect(() => {
    void runs.conversationId;
    pinned = true;
  });

  function userText(r: Run) {
    return r.steps.find((s) => s.kind === "user")?.text ?? "";
  }
  function thinkingMs(r: Run, i: number) {
    const s = r.steps[i];
    return (s.endedAt ?? Date.now()) - s.at;
  }
  /** Le azioni sullo schermo consecutive (3 o più) diventano una striscia di miniature */
  type Pezzo = { kind: "step"; i: number } | { kind: "strip"; ids: string[]; start: number };
  function pezzi(r: Run): Pezzo[] {
    const out: Pezzo[] = [];
    let i = 0;
    while (i < r.steps.length) {
      let j = i;
      while (j < r.steps.length && r.steps[j].kind === "tool" && r.steps[j].category === "computer" && !approvals.forStep(r.steps[j].id)) j++;
      if (j - i >= 3) {
        out.push({ kind: "strip", ids: r.steps.slice(i, j).map((s) => s.id), start: i });
        i = j;
      } else {
        out.push({ kind: "step", i });
        i++;
      }
    }
    return out;
  }
  let openStrips = $state<Record<string, boolean>>({});

  const waiting = (r: Run) => r.status === "running" && !r.steps.some((s) => s.kind === "text" || s.kind === "tool");
</script>

<section class="chat" aria-label="Conversazione">
  <div class="scroll" bind:this={scroller} onscroll={onScroll}>
    <div class="col">
      {#if runs.loading}
        <div class="loading"><div class="skeleton" style="width:60%"></div><div class="skeleton" style="width:85%"></div><div class="skeleton" style="width:40%"></div></div>
      {:else if !runs.current.length}
        <div class="hello">
          <Avatar size={132} />
          <h1>{greeting(settings.s?.userName ?? "")}.</h1>
          <p>Cosa faccio per te{agent && !agent.system ? ` con ${agent.name}` : ""}?</p>
          {#if agent && agent.provider !== "local" && !settings.ready(agent.provider)}
            {#if agent.provider !== "claude-code" && settings.claudeCode?.loggedIn}
              <button class="setup" onclick={() => agents.useSubscription(agent)}>
                <Icon name="user" size={15} /> Usa il tuo abbonamento Claude
              </button>
              <p class="alt">oppure <button class="link" onclick={() => ui.openSettings("providers")}>aggiungi una chiave API</button></p>
            {:else}
              <button class="setup" onclick={() => ui.openSettings("providers")}>
                <Icon name="key" size={15} /> Collega {providerLabel(agent.provider)} per iniziare
              </button>
            {/if}
          {:else}
            <div class="sugg">
              {#each SUGGESTIONS as s (s.text)}
                <button onclick={() => composer?.fill(s.text)}><Icon name={s.icon} size={15} /> {s.text}</button>
              {/each}
            </div>
          {/if}
        </div>
      {:else}
        {#each runs.current as r (r.id)}
          {@const a = agents.byId(r.agentId) ?? agent}
          <article class="turn">
            {#each r.steps.find((s) => s.kind === "user")?.attachments ?? [] as f, fi (fi)}
              <div class="att" title="{f.name} · {fileSize(f.size)}">
                <span class="ext"><Icon name={fileIcon(f.name)} size={15} /></span>
                <span class="att-text"><strong>{f.name}</strong><small>{fileExt(f.name)} · {fileSize(f.size)}{f.kind === "text" ? " · letto come testo" : ""}</small></span>
              </div>
            {/each}
            <div class="q">
              {#if r.steps.find((s) => s.kind === "user")?.viaVoice}<Icon name="mic" size={13} />{/if}
              <p>{userText(r)}</p>
            </div>
            <div class="a" class:selected={runs.selectedRunId === r.id}>
              {#if a}<span class="who"><AgentDisc icon={a.icon} hue={a.hue} size={24} /></span>{/if}
              <div class="body">
                {#each pezzi(r) as pz (pz.kind === "strip" ? pz.ids[0] : r.steps[pz.i].id)}
                  {#if pz.kind === "strip"}
                    {@const steps = r.steps.filter((x) => pz.ids.includes(x.id))}
                    {#if openStrips[pz.ids[0]]}
                      {#each steps as st (st.id)}<ToolCard step={st} />{/each}
                      <button class="link small-link" onclick={() => (openStrips[pz.ids[0]] = false)}>Raggruppa</button>
                    {:else}
                      <button class="strip" onclick={() => (openStrips[pz.ids[0]] = true)} title="Mostra le azioni una per una">
                        <span class="strip-n"><Icon name="target" size={14} /> {steps.length} azioni sullo schermo</span>
                        <span class="strip-shots">
                          {#each steps.filter((x) => (x.result as any)?.screenshotPath).slice(-6) as st (st.id)}
                            <img src={assetUrl((st.result as any).screenshotPath)} alt="" loading="lazy" />
                          {/each}
                        </span>
                        <span class="strip-last">{steps[steps.length - 1].title}</span>
                      </button>
                    {/if}
                  {:else}
                  {@const s = r.steps[pz.i]}
                  {@const i = pz.i}
                  {#if s.kind === "thinking" && showThinking}
                    <button class="think" onclick={() => (openThinking[s.id] = !openThinking[s.id])} aria-expanded={!!openThinking[s.id]}>
                      <Icon name="bulb" size={13} />
                      {s.endedAt ? `Ragionamento · ${duration(thinkingMs(r, i))}` : "Sto ragionando…"}
                      <Icon name={openThinking[s.id] ? "down" : "chevron"} size={12} />
                    </button>
                    {#if openThinking[s.id]}<div class="think-text md">{@html markdown(s.text)}</div>{/if}
                  {:else if s.kind === "text"}
                    <div class="md" class:streaming={r.status === "running" && i === r.steps.length - 1}>{@html markdown(s.text)}</div>
                  {:else if s.kind === "notice"}
                    <p class="notice"><Icon name="info" size={13} /> {s.text}</p>
                  {:else if s.kind === "tool"}
                    <ToolCard step={s} />
                  {/if}
                  {/if}
                {/each}
                {#if waiting(r)}
                  <div class="typing" aria-live="polite"><span></span><span></span><span></span> {runs.progress[r.id] ?? `${avatar.label}…`}</div>
                {/if}
                {#if r.status === "error"}
                  <div class="error">
                    <Icon name="alert" size={15} />
                    <span>{r.error}</span>
                    {#if r.error?.includes("Impostazioni") || r.error?.includes("chiave")}
                      <button class="link" onclick={() => ui.openSettings("providers")}>Apri le impostazioni</button>
                    {/if}
                  </div>
                {:else if r.status === "cancelled"}
                  <p class="meta">Interrotta</p>
                {:else if r.status === "done" || r.status === "refused"}
                  <button class="meta" onclick={() => (runs.selectedRunId = r.id)} title="Mostra i passaggi">
                    {time(r.startedAt)}{r.endedAt ? ` · ${duration(r.endedAt - r.startedAt)}` : ""}{r.servedModel ? ` · ${modelLabel("anthropic", r.servedModel)}` : ""}
                  </button>
                {/if}
              </div>
            </div>
          </article>
        {/each}
      {/if}
    </div>
  </div>
  <div class="foot">
    <div class="col">
      {#each approvals.pending.filter((p) => !runs.order.includes(p.runId)) as p (p.id)}
        <div class="elsewhere"><ApprovalCard request={p} compact /></div>
      {/each}
      <Composer bind:this={composer} />
    </div>
  </div>
</section>

<style>
  .chat {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    background: var(--paper);
  }
  .scroll {
    flex: 1;
    overflow-y: auto;
    padding: 28px 28px 8px;
  }
  .col {
    max-width: 740px;
    margin: 0 auto;
  }
  .foot {
    padding: 8px 28px 20px;
  }
  .elsewhere {
    margin-bottom: 10px;
  }
  .loading {
    display: grid;
    gap: 12px;
    padding-top: 30px;
  }
  .loading .skeleton {
    height: 14px;
  }

  .hello {
    display: grid;
    justify-items: center;
    text-align: center;
    padding: 6vh 0 2vh;
    animation: settle 0.5s var(--ease-out);
  }
  h1 {
    margin: 14px 0 4px;
    font-family: var(--font-serif);
    font-weight: 400;
    font-size: var(--t-display);
    line-height: 1.05;
    letter-spacing: -0.01em;
  }
  .hello > p {
    margin: 0 0 26px;
    font-family: var(--font-serif);
    font-style: italic;
    font-size: 1.35rem;
    color: var(--ink-3);
  }
  .sugg {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
    width: 100%;
  }
  .sugg button {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 13px 14px;
    border-radius: var(--r-lg);
    font-size: var(--t-sm);
    line-height: 1.4;
    text-align: left;
    color: var(--ink-2);
    background: var(--paper-2);
    box-shadow: inset 0 0 0 1px var(--rule-soft);
    transition: background 0.15s, color 0.15s, box-shadow 0.15s;
  }
  .sugg button :global(svg) {
    flex: none;
    margin-top: 1px;
    color: var(--accent);
  }
  .sugg button:hover {
    background: var(--accent-mist);
    color: var(--ink);
    box-shadow: inset 0 0 0 1px var(--accent-wash);
  }
  .setup {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 40px;
    padding: 0 18px;
    border-radius: 999px;
    font-weight: 600;
    color: var(--on-accent);
    background: var(--accent);
  }
  .setup:hover {
    background: var(--accent-hi);
  }
  .alt {
    margin: 10px 0 0;
    font-size: var(--t-sm);
    color: var(--ink-3);
  }

  .turn {
    display: grid;
    gap: 14px;
    margin-bottom: 30px;
    animation: settle 0.35s var(--ease-out);
  }
  .q {
    justify-self: end;
    display: flex;
    align-items: flex-start;
    gap: 6px;
    max-width: 82%;
    padding: 10px 15px;
    border-radius: 18px 18px 6px 18px;
    background: var(--accent-mist);
    box-shadow: inset 0 0 0 1px var(--accent-wash);
    color: var(--ink);
  }
  .q :global(svg) {
    flex: none;
    margin-top: 4px;
    color: var(--accent);
  }
  .q p {
    margin: 0;
    white-space: pre-wrap;
    line-height: 1.55;
    user-select: text;
    -webkit-user-select: text;
    overflow-wrap: anywhere;
  }
  .strip {
    display: grid;
    gap: 6px;
    justify-items: start;
    padding: 9px 12px;
    border-radius: var(--r-md);
    background: var(--paper-2);
    box-shadow: inset 0 0 0 1px var(--rule-soft);
    text-align: left;
  }
  .strip:hover {
    box-shadow: inset 0 0 0 1px var(--accent-wash);
  }
  .strip-n {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: var(--t-sm);
    font-weight: 600;
    color: var(--accent);
  }
  .strip-shots {
    display: flex;
    gap: 4px;
  }
  .strip-shots img {
    width: 64px;
    height: 40px;
    object-fit: cover;
    border-radius: 5px;
    box-shadow: 0 0 0 1px var(--rule);
  }
  .strip-last {
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
  .small-link {
    justify-self: start;
    font-size: var(--t-xs);
  }
  .att {
    justify-self: end;
    display: flex;
    align-items: center;
    gap: 10px;
    max-width: 70%;
    margin-bottom: -6px;
    padding: 8px 14px 8px 8px;
    border-radius: 12px;
    background: var(--paper-2);
    box-shadow: inset 0 0 0 1px var(--rule-soft);
  }
  .att .ext {
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    border-radius: 9px;
    color: var(--accent);
    background: var(--accent-mist);
    flex: none;
  }
  .att-text {
    display: grid;
    min-width: 0;
  }
  .att-text strong {
    font-size: var(--t-sm);
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .att-text small {
    font-size: var(--t-xs);
    color: var(--ink-3);
  }
  .a {
    display: grid;
    grid-template-columns: 24px 1fr;
    gap: 12px;
  }
  .who {
    padding-top: 1px;
  }
  .body {
    display: grid;
    gap: 8px;
    min-width: 0;
    font-size: var(--t-lg);
  }
  .think {
    justify-self: start;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 26px;
    padding: 0 10px;
    border-radius: 999px;
    font-size: var(--t-xs);
    font-weight: 600;
    color: var(--ink-3);
    background: var(--paper-2);
    box-shadow: inset 0 0 0 1px var(--rule-soft);
  }
  .think:hover {
    color: var(--accent);
  }
  .think-text {
    padding: 2px 0 4px 14px;
    border-left: 2px solid var(--accent-wash);
    font-size: var(--t-sm);
    color: var(--ink-3);
  }
  .notice {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
    font-size: var(--t-sm);
    color: var(--ink-3);
    font-style: italic;
  }
  .typing {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: var(--t-sm);
    color: var(--ink-3);
  }
  .typing span {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
    animation: dot 1.2s infinite;
  }
  .typing span:nth-child(2) {
    animation-delay: 0.15s;
  }
  .typing span:nth-child(3) {
    animation-delay: 0.3s;
    margin-right: 6px;
  }
  @keyframes dot {
    0%, 60%, 100% {
      opacity: 0.25;
      transform: none;
    }
    30% {
      opacity: 1;
      transform: translateY(-3px);
    }
  }
  .error {
    display: flex;
    align-items: flex-start;
    flex-wrap: wrap;
    gap: 8px;
    padding: 10px 12px;
    border-radius: var(--r-md);
    font-size: var(--t-sm);
    line-height: 1.45;
    color: var(--danger);
    background: var(--danger-wash);
    user-select: text;
    -webkit-user-select: text;
  }
  .error :global(svg) {
    flex: none;
    margin-top: 1px;
  }
  .error span {
    flex: 1;
    min-width: 200px;
  }
  .meta {
    justify-self: start;
    margin: 0;
    font-size: var(--t-xs);
    color: var(--ink-3);
    opacity: 0;
    transition: opacity 0.15s;
  }
  .turn:hover .meta,
  .a.selected .meta,
  p.meta {
    opacity: 1;
  }
  button.meta:hover {
    color: var(--accent);
  }
  @media (max-width: 1180px) {
    .sugg {
      grid-template-columns: 1fr;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .typing span {
      animation: none;
      opacity: 0.6;
    }
  }
</style>
