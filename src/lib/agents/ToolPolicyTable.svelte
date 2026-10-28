<script lang="ts">
  // Strumenti dell'agente, raggruppati per categoria: attivazione e regola (consenti/chiedi/nega).
  import Icon from "../Icon.svelte";
  import type { Agent, Policy, ToolDef } from "../api";
  import { categoryMeta, RISK } from "../run/toolmeta";
  import { mcp } from "../state/mcp.svelte";

  let { draft = $bindable(), catalog }: { draft: Agent; catalog: ToolDef[] } = $props();

  const groups = $derived(
    Object.entries(
      catalog.reduce<Record<string, ToolDef[]>>((acc, t) => {
        (acc[t.category] ??= []).push(t);
        return acc;
      }, {}),
    ),
  );

  const enabled = (t: ToolDef) => draft.tools.includes(t.name) || draft.tools.includes(t.category) || (draft.tools.includes("mcp") && t.category.startsWith("mcp:"));
  const base = (t: ToolDef): Policy => (t.risk === "read" ? draft.policy.readDefault : t.risk === "write" ? draft.policy.writeDefault : draft.policy.dangerDefault);
  const rule = (t: ToolDef): Policy => draft.policy.perTool[t.name] ?? base(t);

  /** Le categorie diventano nomi singoli, così si può spegnere uno strumento alla volta. */
  function explode() {
    const names = new Set<string>();
    for (const t of catalog) if (enabled(t)) names.add(t.name);
    for (const x of draft.tools) if (x !== "mcp" && !catalog.some((t) => t.category === x) && !catalog.some((t) => t.name === x)) names.add(x);
    draft.tools = [...names];
  }

  function toggle(t: ToolDef, on: boolean) {
    explode();
    draft.tools = on ? [...new Set([...draft.tools, t.name])] : draft.tools.filter((x) => x !== t.name);
  }

  function toggleGroup(tools: ToolDef[], on: boolean) {
    explode();
    const names = tools.map((t) => t.name);
    draft.tools = on ? [...new Set([...draft.tools, ...names])] : draft.tools.filter((x) => !names.includes(x));
  }

  function setRule(t: ToolDef, p: Policy) {
    const next = { ...draft.policy.perTool };
    if (p === base(t)) delete next[t.name];
    else next[t.name] = p;
    draft.policy.perTool = next;
  }

  const risky = $derived(catalog.filter((t) => enabled(t) && t.risk === "danger" && rule(t) === "allow"));
  const RULES: { id: Policy; label: string }[] = [
    { id: "allow", label: "Consenti" },
    { id: "ask", label: "Chiedi" },
    { id: "deny", label: "Nega" },
  ];
</script>

<div class="table">
  {#each groups as [cat, tools] (cat)}
    {@const allOn = tools.every(enabled)}
    <div class="group">
      <label class="ghead">
        <input type="checkbox" checked={allOn} indeterminate={!allOn && tools.some(enabled)} onchange={(e) => toggleGroup(tools, (e.target as HTMLInputElement).checked)} />
        <Icon name={categoryMeta(cat, (id) => mcp.name(id)).icon} size={15} />
        <strong>{categoryMeta(cat, (id) => mcp.name(id)).label}</strong>
      </label>
      {#each tools as t (t.name)}
        <div class="tool" class:off={!enabled(t)}>
          <label class="name">
            <input type="checkbox" checked={enabled(t)} onchange={(e) => toggle(t, (e.target as HTMLInputElement).checked)} />
            <span><strong>{t.label}</strong><small>{t.description}</small></span>
          </label>
          <span class="risk r-{t.risk}">{RISK[t.risk]}</span>
          <div class="rules" role="radiogroup" aria-label="Regola per {t.label}">
            {#each RULES as r (r.id)}
              <button role="radio" aria-checked={rule(t) === r.id} class:on={rule(t) === r.id} class="r-{r.id}" disabled={!enabled(t)} onclick={() => setRule(t, r.id)}>{r.label}</button>
            {/each}
          </div>
        </div>
      {/each}
    </div>
  {/each}
  {#if risky.length}
    <p class="warn"><Icon name="alert" size={15} /> {risky.map((t) => t.label).join(", ")} senza conferma: l'agente potrà agire sul computer senza chiederti nulla.</p>
  {/if}
</div>

<style>
  .table {
    display: grid;
    gap: 12px;
  }
  .group {
    border-radius: var(--r-md);
    box-shadow: inset 0 0 0 1px var(--rule-soft);
    overflow: hidden;
  }
  .ghead {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 9px 12px;
    background: var(--paper-2);
    color: var(--accent);
    font-size: var(--t-sm);
  }
  .ghead strong {
    color: var(--ink);
    font-weight: 650;
  }
  input[type="checkbox"] {
    accent-color: var(--accent);
    margin: 0;
  }
  .tool {
    display: grid;
    grid-template-columns: 1fr auto auto;
    align-items: center;
    gap: 10px;
    padding: 9px 12px;
    border-top: 1px solid var(--rule-soft);
  }
  .tool.off .name span {
    opacity: 0.55;
  }
  .name {
    display: flex;
    align-items: flex-start;
    gap: 9px;
    min-width: 0;
  }
  .name input {
    margin-top: 3px;
  }
  .name span {
    display: grid;
    min-width: 0;
  }
  .name strong {
    font-size: var(--t-sm);
    font-weight: 600;
  }
  .name small {
    font-size: 0.7rem;
    line-height: 1.35;
    color: var(--ink-3);
  }
  .risk {
    font-size: 0.66rem;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    padding: 2px 6px;
    border-radius: 999px;
    color: var(--ink-3);
    background: var(--paper-3);
    white-space: nowrap;
  }
  .risk.r-write {
    color: var(--warn-ink);
    background: oklch(0.72 0.15 75 / 0.15);
  }
  .risk.r-danger {
    color: var(--danger);
    background: var(--danger-wash);
  }
  .rules {
    display: flex;
    padding: 2px;
    border-radius: 8px;
    background: var(--paper-2);
    box-shadow: inset 0 0 0 1px var(--rule-soft);
  }
  .rules button {
    height: 24px;
    padding: 0 8px;
    border-radius: 6px;
    font-size: 0.7rem;
    font-weight: 600;
    color: var(--ink-3);
  }
  .rules button.on {
    background: var(--paper);
    color: var(--ink);
    box-shadow: 0 1px 2px oklch(0.3 0.1 var(--accent-h) / 0.15);
  }
  .rules button.on.r-allow {
    color: var(--ok);
  }
  .rules button.on.r-deny {
    color: var(--danger);
  }
  .rules button:disabled {
    opacity: 0.4;
  }
  .warn {
    display: flex;
    gap: 8px;
    margin: 0;
    padding: 10px 12px;
    border-radius: var(--r-md);
    font-size: var(--t-sm);
    line-height: 1.45;
    color: var(--warn-ink);
    background: oklch(0.72 0.15 75 / 0.12);
  }
  .warn :global(svg) {
    flex: none;
    margin-top: 1px;
  }
</style>
