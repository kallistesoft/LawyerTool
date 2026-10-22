<script lang="ts">
  import Icon from './Icon.svelte'
  import { VERDICT_LABEL, VERDICT_TONE, type Estimated } from './model'

  /** Preliminary verdict from the intake form (imported clients). */
  let { estimate }: { estimate: Estimated } = $props()
  let details = $state(false)

  const crs = $derived('Ok' in estimate.crs ? estimate.crs.Ok : null)
  const icon = { blocking: 'x', check: 'alert', positive: 'check' } as const
  const sections = $derived(
    crs
      ? ([
          ['A. Capital humain', crs.core.subtotal],
          ['B. Époux ou conjoint de fait', crs.with_spouse ? crs.spouse.subtotal : null],
          ['C. Transférabilité', crs.transferability.subtotal],
          ['D. Points supplémentaires', crs.additional.subtotal],
          ['Marché du travail', crs.labour_market?.subtotal ?? null],
        ] as [string, number | null][]).filter(([, v]) => v !== null)
      : [],
  )
</script>

<div class="card estimate {VERDICT_TONE[estimate.verdict]}">
  <div class="head">
    <div>
      <div class="label">Pré-évaluation d’après le formulaire</div>
      <div class="verdict">
        <span class="badge {VERDICT_TONE[estimate.verdict]}">{VERDICT_LABEL[estimate.verdict]}</span>
        {#if crs}<span class="hint">Score SCG estimé</span> <strong class="num">≈ {crs.total}</strong>{/if}
      </div>
    </div>
    <button class="ghost small" onclick={() => (details = !details)} aria-expanded={details}>
      {details ? 'Masquer' : 'Hypothèses et détail'}
    </button>
  </div>

  <ul class="reasons">
    {#each estimate.reasons as r, i (i)}
      <li class={r.kind}><Icon name={icon[r.kind]} size={14} /><span>{r.text}</span></li>
    {/each}
  </ul>

  {#if details}
    <div class="details">
      {#if sections.length}
        <table>
          <tbody>
            {#each sections as [label, points] (label)}
              <tr><td>{label}</td><td class="num">{points}</td></tr>
            {/each}
          </tbody>
        </table>
      {/if}
      <ul class="assumptions hint">
        {#each estimate.assumptions as a, i (i)}<li>{a}</li>{/each}
      </ul>
    </div>
  {/if}
</div>

<style>
  .estimate {
    margin-bottom: 1rem;
    border-left: 4px solid var(--border);
  }
  .estimate.ok {
    border-left-color: var(--ok);
  }
  .estimate.warn {
    border-left-color: var(--warn);
  }
  .estimate.err {
    border-left-color: var(--err);
  }
  .head {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 1rem;
  }
  .verdict {
    display: flex;
    align-items: baseline;
    gap: 0.6rem;
    margin-top: 0.35rem;
  }
  .verdict strong {
    font-size: 1.4rem;
  }
  .reasons {
    list-style: none;
    margin: 0.8rem 0 0;
    padding: 0;
  }
  .reasons li {
    display: flex;
    gap: 0.5rem;
    align-items: flex-start;
    padding: 0.3rem 0;
    font-size: 0.92rem;
  }
  .reasons li.blocking :global(svg) {
    color: var(--err);
  }
  .reasons li.check :global(svg) {
    color: var(--warn);
  }
  .reasons li.positive :global(svg) {
    color: var(--ok);
  }
  .reasons :global(svg) {
    flex: none;
    margin-top: 0.2rem;
  }
  .details {
    margin-top: 0.8rem;
    padding-top: 0.8rem;
    border-top: 1px solid var(--border);
  }
  table {
    width: auto;
    margin-bottom: 0.6rem;
  }
  td {
    padding: 0.15rem 1.2rem 0.15rem 0;
  }
  .assumptions {
    margin: 0;
    padding-left: 1.2rem;
    font-size: 0.86rem;
  }
</style>
