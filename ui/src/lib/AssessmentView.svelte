<script lang="ts">
  import { onMount } from 'svelte'
  import { assess, clientGet, deidPreview, errorMessage, exportReport, settingsGet, type Redaction } from './api'
  import Icon from './Icon.svelte'
  import ImportNotes from './ImportNotes.svelte'
  import {
    localToday, PROGRAM, STATUS_LABEL, STATUS_TONE,
    type Assessment, type Check, type Cite, type CrsResult, type Flag, type ProgramResult, type Section,
  } from './model'
  import PageHeader from './PageHeader.svelte'

  let { id, onEdit, onBack }: { id: number; onEdit: (id: number) => void; onBack: () => void } = $props()

  let on = $state(localToday())
  let result = $state<Assessment | null>(null)
  let error: string | null = $state(null)
  let reviewed: Record<string, boolean> = $state({})
  let name = $state('Client')
  let fileRef: string | null = $state(null)
  let notes: string[] = $state([])
  let tab = $state<'score' | 'programs'>('score')

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
  let exported: string | null = $state(null)
  let exportError: string | null = $state(null)
  let toast: string | null = $state(null)
  async function doExport(format: 'pdf' | 'docx') {
    exportError = null
    try {
      const path = await exportReport(id, on, format, Object.keys(reviewed).filter((k) => reviewed[k]))
      if (path) {
        exported = path
        toast = format === 'pdf' ? 'Rapport PDF enregistré' : 'Document Word enregistré'
        setTimeout(() => (toast = null), 3000)
      }
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
  onMount(() => {
    clientGet(id)
      .then((c) => ((name = c.identity.full_name), (fileRef = c.identity.file_reference), (notes = c.notes)))
      .catch(() => {})
    settingsGet()
      .then((s) => (outputDir = s.output_dir?.path ?? null))
      .catch(() => {})
    run()
  })

  const crs = $derived(result && 'Ok' in result.crs ? result.crs.Ok : null)
  const programs = $derived(result && 'Ok' in result.eligibility ? result.eligibility.Ok : null)
  const reviews = $derived((crs?.flags ?? []).flatMap((f: Flag) => ('ManualReview' in f ? [f.ManualReview] : [])))
  const otherFlags = $derived(
    (crs?.flags ?? []).flatMap((f: Flag) => ('ExpiredLanguageTest' in f ? [`Test de langue expiré : ${f.ExpiredLanguageTest.which}`] : [])),
  )
  const done = $derived(reviews.filter((r) => reviewed[r.id]).length)
  const allReviewed = $derived(reviews.every((r) => reviewed[r.id]))
  const latest = $derived(result?.draws[0] ?? null)
  const gap = $derived(crs && latest ? crs.total - latest.min_crs : null)

  const sections = (c: CrsResult): [string, Section | null, number][] => [
    ['A. Capital humain', c.core, c.with_spouse ? 460 : 500],
    ['B. Époux ou conjoint de fait', c.with_spouse ? c.spouse : null, 40],
    ['C. Transférabilité des compétences', c.transferability, 100],
    ['D. Points supplémentaires', c.additional, 600],
    ['Intégration au marché du travail', c.labour_market, c.with_spouse ? 140 : 150],
  ]
  /** The most telling requirement of a program: first blocker, else first missing input. */
  function reason(p: ProgramResult): string | null {
    const r =
      p.requirements.find((r) => r.check === 'NotMet') ??
      p.requirements.find((r) => r.check === 'NeedsInput')
    if (r) return r.detail
    const checks = p.requirements.filter((r) => r.check === 'LawyerCheck').length
    return checks ? `${checks} point${checks > 1 ? 's' : ''} à confirmer par l’avocat` : null
  }
  const ICON: Record<Check, string> = { Met: '✓', NotMet: '✗', NeedsInput: '?', LawyerCheck: '⚖' }
  const CHECK_LABEL: Record<Check, string> = { Met: 'Satisfait', NotMet: 'Non satisfait', NeedsInput: 'À compléter', LawyerCheck: 'À vérifier par l’avocat' }
  const cite = (c: Cite) => `${c.source}${c.table !== null ? ` · tableau ${c.table}` : ''}${c.row !== null ? `, ligne ${c.row}` : ''} : ${c.text}`
  const fileName = (p: string) => p.split(/[\\/]/).pop() ?? p
  const dirName = (p: string) => p.split(/[\\/]/).filter(Boolean).pop() ?? p
</script>

<PageHeader
  title={name}
  crumbs={[{ label: 'Clients', go: onBack }]}
  subtitle={`${fileRef ? `Dossier ${fileRef} · ` : ''}Évaluation au ${on}${result?.grid_id ? ` · grille ${result.grid_id}` : ''}`}
>
  {#snippet actions()}
    <label class="date">Date d’évaluation <input type="date" bind:value={on} onchange={run} /></label>
    <button onclick={() => onEdit(id)}><Icon name="edit" />Modifier le profil</button>
  {/snippet}
</PageHeader>

<ImportNotes {notes} />

{#if error}<p class="callout err">{error}</p>{/if}

{#if result}
  <div class="hero">
    <!-- Score -->
    <div class="card score-card">
      <div class="label">Score SCG</div>
      {#if crs}
        <div class="big num">{crs.total}<span class="of">/ 1200</span></div>
        <div class="hint">{crs.with_spouse ? 'Avec époux ou conjoint' : 'Sans époux (ou époux non pris en compte)'}</div>
        {#if latest && gap !== null}
          <div class="gap {gap >= 0 ? 'up' : 'down'}">
            <strong>{gap >= 0 ? '+' : ''}{gap}</strong> par rapport au dernier tirage
            <div class="hint">{latest.name} · {latest.date} · seuil {latest.min_crs}</div>
          </div>
        {:else}
          <div class="hint gap-none">Aucun tirage enregistré : lancez une vérification dans Veille des règles.</div>
        {/if}
      {:else if 'Err' in result.crs}
        <div class="big muted">—</div>
        <p class="error">{result.crs.Err}</p>
        <button class="small" onclick={() => onEdit(id)}>Compléter le profil</button>
      {/if}
    </div>

    <!-- Eligibility -->
    <div class="card">
      <div class="label">Admissibilité aux programmes</div>
      {#if programs}
        <ul class="progs">
          {#each programs as p (p.program)}
            <li>
              <div class="prog-head">
                <span>{PROGRAM[p.program]}</span>
                <span class="badge {STATUS_TONE[p.status]}">{STATUS_LABEL[p.status]}</span>
              </div>
              {#if reason(p)}<div class="hint">{reason(p)}</div>{/if}
            </li>
          {/each}
        </ul>
      {:else if 'Err' in result.eligibility}
        <p class="error">{result.eligibility.Err}</p>
      {/if}
    </div>

    <!-- Review & export -->
    <div class="card">
      <div class="label">Révision et export</div>
      {#if reviews.length}
        <div class="progress" title="Points de révision obligatoires">
          <div class="bar"><span style="width: {(done / reviews.length) * 100}%"></span></div>
          <span class={allReviewed ? 'badge ok' : 'badge warn'}>Révision {done}/{reviews.length}</span>
        </div>
      {:else}
        <p><span class="badge ok"><Icon name="check" size={12} />Aucun point de révision obligatoire</span></p>
      {/if}
      <div class="actions export">
        <button class="primary" disabled={!allReviewed} onclick={() => doExport('pdf')} title={allReviewed ? '' : 'Cochez d’abord chaque point de révision'}>
          <Icon name="download" />PDF
        </button>
        <button disabled={!allReviewed} onclick={() => doExport('docx')} title={allReviewed ? '' : 'Cochez d’abord chaque point de révision'}>
          <Icon name="download" />Word
        </button>
      </div>
      <p class="hint">
        {#if outputDir}Enregistré dans <strong>{dirName(outputDir)}</strong> (dossier de sortie).{:else}L’emplacement vous sera demandé.{/if}
        Le fichier exporté n’est pas chiffré.
      </p>
      {#if exported}<p class="saved"><Icon name="check" size={14} /> {fileName(exported)}</p>{/if}
      {#if exportError}<p class="error">{exportError}</p>{/if}
    </div>
  </div>

  {#if reviews.length}
    <div class="callout warn review">
      <div class="card-title">
        <h3><Icon name="scale" /> Révision obligatoire par l’avocat</h3>
        <span class="badge warn">{done}/{reviews.length}</span>
      </div>
      <p class="hint">Ces valeurs reposent sur une décision prise sur une divergence entre sources officielles. L’export reste bloqué tant qu’elles ne sont pas cochées.</p>
      {#each reviews as r (r.id)}
        <label class="check"><input type="checkbox" bind:checked={reviewed[r.id]} /> <span><strong>{r.id}</strong> — {r.reason}</span></label>
      {/each}
    </div>
  {/if}
  {#each otherFlags as f (f)}<p class="callout warn">{f}</p>{/each}

  <div class="tabs" role="tablist">
    <button role="tab" class:active={tab === 'score'} onclick={() => (tab = 'score')}>Détail du score</button>
    <button role="tab" class:active={tab === 'programs'} onclick={() => (tab = 'programs')}>Admissibilité détaillée</button>
  </div>

  {#if tab === 'score'}
    {#if crs}
      {#each sections(crs) as [title, s, max] (title)}
        {#if s}
          <details class="card section" open={title.startsWith('A') || title.startsWith('C') || title.startsWith('D')}>
            <summary>
              <span class="sec-title">{title}</span>
              <span class="sec-bar"><span style="width: {Math.min(100, (s.subtotal / max) * 100)}%"></span></span>
              <span class="sec-total num"><strong>{s.subtotal}</strong> <span class="hint">/ {max}</span></span>
            </summary>
            {#if s.lines.length === 0}
              <p class="hint">Aucun point dans cette section.</p>
            {:else}
              <table>
                <tbody>
                  {#each s.lines as l (l.label)}
                    <tr>
                      <td>
                        {l.label}
                        {#if l.cites.length}
                          <details class="sources">
                            <summary class="hint">Sources ({l.cites.length})</summary>
                            {#each l.cites as c, i (i)}<div class="cite">{cite(c)}</div>{/each}
                          </details>
                        {/if}
                      </td>
                      <td class="pts num">{l.points}</td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            {/if}
          </details>
        {/if}
      {/each}
    {:else if 'Err' in result.crs}
      <p class="callout err">{result.crs.Err}</p>
    {/if}
  {:else if programs}
    {#each programs as p (p.program)}
      <div class="card">
        <div class="card-title">
          <h3>{PROGRAM[p.program]}</h3>
          <span class="badge {STATUS_TONE[p.status]}">{STATUS_LABEL[p.status]}</span>
        </div>
        {#if p.fsw_points}
          <p class="hint">
            Grille de sélection : <strong>{p.fsw_points.min === p.fsw_points.max ? p.fsw_points.min : `${p.fsw_points.min}–${p.fsw_points.max}`} / 100</strong>
            (note de passage {p.fsw_points.pass_mark})
          </p>
        {/if}
        <table>
          <tbody>
            {#each p.requirements as r (r.id)}
              <tr class="req {r.check}">
                <td class="icon" title={CHECK_LABEL[r.check]}>{ICON[r.check]}</td>
                <td>
                  {r.detail}
                  {#if r.cites.length}
                    <details class="sources">
                      <summary class="hint">Sources ({r.cites.length})</summary>
                      {#each r.cites as c, i (i)}<div class="cite">{cite(c)}</div>{/each}
                    </details>
                  {/if}
                </td>
                <td class="state"><span class="hint">{CHECK_LABEL[r.check]}</span></td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/each}
  {:else if 'Err' in result.eligibility}
    <p class="callout err">{result.eligibility.Err}</p>
  {/if}

  <div class="secondary">
    {#if result.draws.length}
      <details class="card subtle">
        <summary><strong>Derniers tirages</strong> <span class="hint">({result.draws.length})</span></summary>
        <table>
          <thead><tr><th>Date</th><th>Type</th><th class="num">Seuil</th><th class="num">Écart</th></tr></thead>
          <tbody>
            {#each result.draws as d (d.number)}
              <tr>
                <td>{d.date}</td><td>{d.name}</td><td class="num">{d.min_crs}</td>
                <td class="num">
                  {#if crs}<span class="badge {crs.total >= d.min_crs ? 'ok' : 'err'}">{crs.total - d.min_crs >= 0 ? '+' : ''}{crs.total - d.min_crs}</span>{/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
        <p class="hint">Comparez au type de tirage visé : chaque catégorie a son propre seuil.</p>
      </details>
    {/if}
    <details class="card subtle">
      <summary><strong>Envoi à Claude (aperçu)</strong> <span class="hint">· rien n’est envoyé à cette étape</span></summary>
      <p class="hint">
        Pour rédiger la note, seul ce contenu serait transmis : profil et résultats, sans nom, coordonnées ni numéros d’identification.
      </p>
      <button class="small" onclick={showPreview}><Icon name="shield" size={14} />Afficher l’aperçu</button>
      {#if previewError}<p class="error">{previewError}</p>{/if}
      {#if preview}
        <p class="hint">
          {preview.bytes.toLocaleString('fr-CA')} octets ·
          {preview.redactions.length === 0
            ? 'aucun identifiant trouvé dans le contenu'
            : `${preview.redactions.length} remplacement(s) : ${[...new Set(preview.redactions.map((r) => r.placeholder))].join(', ')}`}
        </p>
        <pre class="payload">{preview.payload}</pre>
      {/if}
    </details>
  </div>
{/if}

{#if toast}<div class="toast" role="status">{toast}</div>{/if}

<style>
  .date {
    margin: 0;
  }
  .date input {
    margin-top: 0.15rem;
  }
  .hero {
    display: grid;
    grid-template-columns: minmax(14rem, 1fr) minmax(18rem, 1.5fr) minmax(16rem, 1.1fr);
    gap: 1rem;
    margin-bottom: 1rem;
  }
  @media (max-width: 1100px) {
    .hero {
      grid-template-columns: 1fr 1fr;
    }
  }
  @media (max-width: 760px) {
    .hero {
      grid-template-columns: 1fr;
    }
  }
  .hero > .card {
    margin: 0;
  }
  .label {
    font-size: 0.75rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--muted);
    margin-bottom: 0.5rem;
  }
  .score-card {
    background: linear-gradient(160deg, var(--accent-soft), var(--surface) 70%);
  }
  .big {
    font-size: 3.2rem;
    font-weight: 800;
    line-height: 1;
    color: var(--accent);
    margin: 0.2rem 0 0.4rem;
  }
  .of {
    font-size: 1rem;
    font-weight: 600;
    color: var(--muted);
    margin-left: 0.35rem;
  }
  .gap {
    margin-top: 0.8rem;
    padding: 0.55rem 0.7rem;
    border-radius: var(--radius-sm);
    font-size: 0.9rem;
  }
  .gap.up {
    background: var(--ok-soft);
    color: var(--ok);
  }
  .gap.down {
    background: var(--err-soft);
    color: var(--err);
  }
  .gap .hint {
    color: var(--muted);
  }
  .gap-none {
    margin-top: 0.8rem;
  }
  .progs {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .progs li {
    padding: 0.5rem 0;
    border-bottom: 1px solid var(--border);
  }
  .progs li:last-child {
    border-bottom: none;
  }
  .prog-head {
    display: flex;
    justify-content: space-between;
    gap: 0.6rem;
    font-weight: 600;
  }
  .progress {
    display: flex;
    gap: 0.6rem;
    align-items: center;
    margin-bottom: 0.4rem;
  }
  .bar {
    flex: 1;
    height: 0.45rem;
    border-radius: 999px;
    background: var(--border);
    overflow: hidden;
  }
  .bar span {
    display: block;
    height: 100%;
    background: var(--ok);
    transition: width 0.2s;
  }
  .export {
    margin: 0.8rem 0 0.4rem;
  }
  .saved {
    display: flex;
    gap: 0.35rem;
    align-items: center;
    color: var(--ok);
    font-weight: 600;
    font-size: 0.88rem;
    word-break: break-all;
  }
  .review h3 {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }
  .tabs {
    display: flex;
    gap: 0.3rem;
    border-bottom: 1px solid var(--border);
    margin: 1.4rem 0 1rem;
  }
  .tabs button {
    border: none;
    background: none;
    border-bottom: 2px solid transparent;
    border-radius: 0;
    padding: 0.55rem 0.9rem;
    color: var(--muted);
  }
  .tabs button.active {
    color: var(--accent);
    border-bottom-color: var(--accent);
  }
  .tabs button:hover:not(:disabled) {
    background: none;
    color: var(--text);
  }
  .section > summary {
    display: grid;
    grid-template-columns: 1fr minmax(6rem, 14rem) 6.5rem;
    gap: 1rem;
    align-items: center;
    list-style: none;
  }
  .section > summary::-webkit-details-marker {
    display: none;
  }
  .section[open] > summary {
    margin-bottom: 0.6rem;
  }
  .sec-title {
    font-weight: 650;
  }
  .sec-bar {
    height: 0.4rem;
    border-radius: 999px;
    background: var(--border);
    overflow: hidden;
  }
  .sec-bar span {
    display: block;
    height: 100%;
    background: var(--accent-2);
  }
  .sec-total {
    text-align: right;
  }
  .pts {
    text-align: right;
    width: 5rem;
    font-weight: 650;
  }
  .sources {
    margin-top: 0.2rem;
  }
  .sources summary {
    font-size: 0.78rem;
  }
  .cite {
    font-size: 0.76rem;
    color: var(--muted);
    margin: 0.2rem 0 0 0.8rem;
  }
  .icon {
    width: 1.8rem;
    text-align: center;
    font-weight: 800;
  }
  .state {
    width: 9rem;
    text-align: right;
  }
  tr.NotMet .icon { color: var(--err); }
  tr.Met .icon { color: var(--ok); }
  tr.NeedsInput .icon { color: var(--warn); }
  tr.LawyerCheck .icon { color: var(--info); }
  tr.NotMet td { background: color-mix(in srgb, var(--err-soft) 55%, transparent); }
  tr.NeedsInput td { background: color-mix(in srgb, var(--warn-soft) 55%, transparent); }
  .secondary {
    margin-top: 1.5rem;
  }
  .secondary summary {
    padding: 0.1rem 0;
  }
  .payload {
    max-height: 24rem;
    overflow: auto;
    font-size: 0.75rem;
    padding: 0.7rem;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
  }
</style>
