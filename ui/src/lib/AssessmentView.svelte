<script lang="ts">
  import { onMount } from 'svelte'
  import { assess, deidPreview, errorMessage, exportReport, settingsGet, type Redaction } from './api'
  import { PROGRAM, type Assessment, type Check, type Cite, type CrsResult, type Flag, type Section } from './model'

  let { id, onEdit, onBack }: { id: number; onEdit: (id: number) => void; onBack: () => void } = $props()

  let on = $state(new Date().toISOString().slice(0, 10))
  let result = $state<Assessment | null>(null)
  let error: string | null = $state(null)
  let reviewed: Record<string, boolean> = $state({})

  let preview = $state<{ payload: string; bytes: number; redactions: Redaction[] } | null>(null)
  let previewError: string | null = $state(null)
  async function showPreview() {
    previewError = null
    try {
      preview = await deidPreview(id, on)
    } catch (e) {
      previewError = errorMessage(e)
    }
  }

  let outputDir: string | null = $state(null)
  settingsGet()
    .then((s) => (outputDir = s.output_dir?.path ?? null))
    .catch(() => {})
  let exported: string | null = $state(null)
  let exportError: string | null = $state(null)
  async function doExport(format: 'pdf' | 'docx') {
    exportError = null
    exported = null
    try {
      exported = await exportReport(id, on, format, Object.keys(reviewed).filter((k) => reviewed[k]))
    } catch (e) {
      exportError = errorMessage(e)
    }
  }

  async function run() {
    preview = null
    error = null
    try {
      result = await assess(id, on)
      reviewed = {}
    } catch (e) {
      error = errorMessage(e)
    }
  }
  onMount(run)

  const crs = $derived(result && 'Ok' in result.crs ? result.crs.Ok : null)
  const programs = $derived(result && 'Ok' in result.eligibility ? result.eligibility.Ok : null)
  const reviews = $derived(
    (crs?.flags ?? []).flatMap((f: Flag) => ('ManualReview' in f ? [f.ManualReview] : [])),
  )
  const otherFlags = $derived(
    (crs?.flags ?? []).flatMap((f: Flag) => ('ExpiredLanguageTest' in f ? [`Test de langue expiré : ${f.ExpiredLanguageTest.which}`] : [])),
  )
  const allReviewed = $derived(reviews.every((r) => reviewed[r.id]))

  const sections = (c: CrsResult): [string, Section | null][] => [
    ['A. Capital humain', c.core],
    ['B. Époux ou conjoint de fait', c.with_spouse ? c.spouse : null],
    ['C. Transférabilité des compétences', c.transferability],
    ['D. Points supplémentaires', c.additional],
    ['Intégration au marché du travail', c.labour_market],
  ]
  const ICON: Record<Check, string> = { Met: '✓', NotMet: '✗', NeedsInput: '?', LawyerCheck: '⚖' }
  const STATUS = { Eligible: 'Admissible (vérifications de l’avocat à confirmer)', NotEligible: 'Non admissible', NeedsInput: 'Renseignements manquants' }
  const cite = (c: Cite) => `${c.source}${c.table !== null ? ` · tableau ${c.table}` : ''}${c.row !== null ? `, ligne ${c.row}` : ''} : ${c.text}`
</script>

<section>
  <div class="row between">
    <h2>Évaluation</h2>
    <span>
      <button class="link" onclick={() => onEdit(id)}>Modifier le profil</button>
      · <button class="link" onclick={onBack}>Liste des clients</button>
    </span>
  </div>
  <div class="row">
    <label>Date d’évaluation <input type="date" bind:value={on} /></label>
    <button onclick={run}>Recalculer</button>
  </div>
  {#if error}<p class="error">{error}</p>{/if}

  {#if result}
    <p class="hint">Grille officielle : {result.grid_id ?? 'aucune'} · calcul déterministe ; chaque point cite sa source.</p>

    {#if reviews.length}
      <div class="warn">
        <p><strong>Révision obligatoire par l’avocat ({reviews.filter((r) => reviewed[r.id]).length}/{reviews.length})</strong></p>
        {#each reviews as r (r.id)}
          <label class="check"><input type="checkbox" bind:checked={reviewed[r.id]} /> <span><strong>{r.id}</strong> : {r.reason}</span></label>
        {/each}
        {#if !allReviewed}<p class="hint">La note ne pourra pas être approuvée tant que ces points ne sont pas révisés.</p>{/if}
      </div>
    {/if}
    {#each otherFlags as f (f)}<p class="warn">{f}</p>{/each}

    <h3>Système de classement global (SCG)</h3>
    {#if crs}
      <p class="total">{crs.total} points <span class="hint">({crs.with_spouse ? 'avec' : 'sans'} époux)</span></p>
      {#each sections(crs) as [title, s] (title)}
        {#if s}
          <details open={title.startsWith('A')}>
            <summary>{title} : <strong>{s.subtotal}</strong></summary>
            <table>
              <tbody>
                {#each s.lines as l (l.label)}
                  <tr>
                    <td>{l.label}</td>
                    <td class="num">{l.points}</td>
                    <td class="cites">{#each l.cites as c, i (i)}<div>{cite(c)}</div>{/each}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </details>
        {/if}
      {/each}
    {:else if result && 'Err' in result.crs}
      <p class="error">{result.crs.Err}</p>
    {/if}

    {#if crs && result.draws.length}
      <h3>Derniers tirages</h3>
      <table>
        <thead><tr><th>Date</th><th>Type</th><th>Note minimale</th><th>Écart</th></tr></thead>
        <tbody>
          {#each result.draws as d (d.number)}
            <tr>
              <td>{d.date}</td><td>{d.name}</td><td>{d.min_crs}</td>
              <td class={crs.total >= d.min_crs ? 'Eligible' : 'NotEligible'}>{crs.total - d.min_crs >= 0 ? '+' : ''}{crs.total - d.min_crs}</td>
            </tr>
          {/each}
        </tbody>
      </table>
      <p class="hint">Comparez au type de tirage visé : chaque catégorie a son propre seuil.</p>
    {/if}

    <h3>Admissibilité aux programmes</h3>
    {#if programs}
      {#each programs as p (p.program)}
        <details open>
          <summary>{PROGRAM[p.program]} : <strong class={p.status}>{STATUS[p.status]}</strong></summary>
          {#if p.fsw_points}
            <p>Grille de sélection : {p.fsw_points.min === p.fsw_points.max ? p.fsw_points.min : `${p.fsw_points.min}–${p.fsw_points.max}`} / 100 (seuil {p.fsw_points.pass_mark})</p>
          {/if}
          <table>
            <tbody>
              {#each p.requirements as r (r.id)}
                <tr class={r.check}>
                  <td class="icon">{ICON[r.check]}</td>
                  <td>{r.detail}</td>
                  <td class="cites">{#each r.cites as c, i (i)}<div>{cite(c)}</div>{/each}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </details>
      {/each}
    {:else if result && 'Err' in result.eligibility}
      <p class="error">{result.eligibility.Err}</p>
    {/if}
    <h3>Exporter l’évaluation</h3>
    <p class="hint">
      Rapport brouillon pour l’avocat, avec les sources citées. Le fichier exporté n’est pas chiffré.
      {#if outputDir}Enregistré dans le dossier de sortie : <code>{outputDir}</code>.{/if}
    </p>
    <div class="row">
      <button disabled={!allReviewed} onclick={() => doExport('pdf')}>{outputDir ? 'Exporter en PDF dans le dossier de sortie' : 'Exporter en PDF'}</button>
      <button disabled={!allReviewed} onclick={() => doExport('docx')}>{outputDir ? 'Exporter en Word dans le dossier de sortie' : 'Exporter en Word'}</button>
    </div>
    {#if !allReviewed}<p class="hint">Cochez d’abord chaque point de révision obligatoire.</p>{/if}
    {#if exported}<p>Enregistré : {exported}</p>{/if}
    {#if exportError}<p class="error">{exportError}</p>{/if}

    <h3>Envoi à Claude</h3>
    <p class="hint">
      Pour rédiger la note, seul ce contenu serait transmis : profil et résultats, sans nom, coordonnées ni numéros
      d’identification. Rien n’est envoyé à cette étape.
    </p>
    <button onclick={showPreview}>Aperçu de l’envoi à Claude</button>
    {#if previewError}<p class="error">{previewError}</p>{/if}
    {#if preview}
      <p>
        {preview.bytes.toLocaleString('fr-CA')} octets ·
        {preview.redactions.length === 0
          ? 'aucun identifiant trouvé dans le contenu'
          : `${preview.redactions.length} remplacement(s) : ${[...new Set(preview.redactions.map((r) => r.placeholder))].join(', ')}`}
      </p>
      <pre class="payload">{preview.payload}</pre>
    {/if}
  {/if}
</section>

<style>
  .payload {
    max-height: 24rem;
    overflow: auto;
    font-size: 0.75rem;
    padding: 0.6rem;
    border: 1px solid color-mix(in srgb, currentColor 20%, transparent);
    border-radius: 4px;
  }
  .total {
    font-size: 2rem;
    margin: 0.3rem 0 1rem;
  }
  .num {
    text-align: right;
    font-variant-numeric: tabular-nums;
    width: 4rem;
  }
  .icon {
    width: 1.5rem;
    text-align: center;
  }
  .cites {
    font-size: 0.72rem;
    opacity: 0.7;
  }
  tr.NotMet td.icon,
  .NotEligible {
    color: #b00020;
  }
  tr.Met td.icon,
  .Eligible {
    color: #1e7d32;
  }
  tr.NeedsInput td.icon,
  .NeedsInput {
    color: #c77700;
  }
  tr.LawyerCheck td.icon {
    color: #2b6cb0;
  }
</style>
