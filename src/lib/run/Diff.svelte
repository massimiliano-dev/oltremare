<script lang="ts">
  let { diff, max = 40 }: { diff: string; max?: number } = $props();
  let all = $state(false);
  const lines = $derived(diff.split("\n").filter((l) => !l.startsWith("---") && !l.startsWith("+++")));
  const shown = $derived(all ? lines : lines.slice(0, max));
</script>

<pre class="diff">{#each shown as l, i (i)}<span class:add={l.startsWith("+")} class:del={l.startsWith("-")} class:hunk={l.startsWith("@@")}>{l || " "}</span>{"\n"}{/each}</pre>
{#if lines.length > max}
  <button class="more" onclick={() => (all = !all)}>{all ? "Mostra meno" : `Mostra tutto (${lines.length} righe)`}</button>
{/if}

<style>
  .diff {
    margin: 0;
    max-height: 340px;
    overflow: auto;
    padding: 8px 0;
    border-radius: var(--r-md);
    background: var(--paper-2);
    box-shadow: inset 0 0 0 1px var(--rule-soft);
    font-family: var(--font-mono);
    font-size: 0.76rem;
    line-height: 1.55;
    user-select: text;
    -webkit-user-select: text;
  }
  span {
    display: inline-block;
    min-width: 100%;
    padding: 0 12px;
    white-space: pre;
  }
  .add {
    background: oklch(0.6 0.14 152 / 0.14);
    color: var(--ok);
  }
  .del {
    background: oklch(0.6 0.17 25 / 0.12);
    color: var(--danger);
  }
  .hunk {
    color: var(--ink-3);
  }
  .more {
    margin-top: 4px;
    font-size: var(--t-xs);
    font-weight: 600;
    color: var(--accent);
  }
</style>
