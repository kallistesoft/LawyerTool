<script lang="ts">
  import { onMount } from 'svelte'
  import {
    auditInputsGet, auditInputsSet, clientClearNotes, clientDelete, clientGet, clientSave, errorMessage,
    type AuditInputs,
  } from './api'
  import Icon from './Icon.svelte'
  import ImportNotes from './ImportNotes.svelte'
  import LanguageFields from './LanguageFields.svelte'
  import PageHeader from './PageHeader.svelte'
  import {
    CDN_EDUCATION, CREDENTIAL, EDUCATION, MARITAL, newApplicant, newLanguageTest,
    type Applicant, type CanadianEducation, type Credential, type EducationLevel, type Identity, type MaritalStatus,
  } from './model'
  import TriState from './TriState.svelte'

  let { id, onSaved, onAssess, onBack }: {
    id: number | null
    onSaved: (id: number) => void
    onAssess: (id: number) => void
    onBack: () => void
  } = $props()

  let identity: Identity = $state({ full_name: '', email: null, phone: null, file_reference: null })
  let a: Applicant = $state(newApplicant())
  let clientId: number | null = $state(null)
  let problems: string[] = $state([])
  let notes: string[] = $state([])
  let audit: AuditInputs | null = $state(null)
  const REGULATED = [
    'n’est pas réglementée',
    'est peut-être réglementée (CNP à préciser impérativement)',
    'est réglementée au Canada',
    'est réglementée en Alberta',
    'est réglementée au Nouveau-Brunswick',
  ]
  let error: string | null = $state(null)
  let saved = $state(false)
  let confirmDelete = $state(false)
  let toast: string | null = $state(null)
  // Unsaved-changes indicator: compare with the last loaded/saved state.
  let baseline = $state('')
  const snapshot = () => JSON.stringify([identity, a])
  const dirty = $derived(baseline !== '' && snapshot() !== baseline)

  onMount(() => {
    clientId = id
    if (id === null) baseline = snapshot()
    if (id !== null) {
      clientGet(id)
        .then((c) => {
          identity = c.identity
          a = c.profile
          notes = c.notes
          auditInputsGet(c.id).then((i) => (audit = i)).catch(() => {})
          baseline = snapshot()
        })
        .catch((e) => (error = errorMessage(e)))
    }
  })

  const hasPartner = (m: MaritalStatus) => m === 'married' || m === 'common_law'
  $effect(() => {
    if (hasPartner(a.marital_status) && !a.spouse) {
      a.spouse = { is_canadian_citizen_or_pr: false, accompanying: true, education: 'secondary', canadian_work_years: 0, language: null }
    }
    if (!hasPartner(a.marital_status) && a.spouse) a.spouse = null
  })
  const french = (t: string) => t === 'tef_canada' || t === 'tcf_canada'

  async function save(then?: (id: number) => void) {
    error = null
    saved = false
    // Our own French checks (the WebView's built-in messages follow the OS language).
    if (!identity.full_name.trim()) {
      error = 'Le nom complet est obligatoire.'
      return
    }
    if (identity.email && !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(identity.email)) {
      error = 'Adresse courriel invalide.'
      return
    }
    try {
      const r = await clientSave(clientId, identity, a)
      clientId = r.id
      if (audit) await auditInputsSet(r.id, audit.lawyer)
      problems = r.problems
      saved = true
      baseline = snapshot()
      toast = r.problems.length ? 'Enregistré comme brouillon' : 'Dossier enregistré'
      setTimeout(() => (toast = null), 2500)
      onSaved(r.id)
      if (then && r.problems.length === 0) then(r.id)
    } catch (e) {
      error = errorMessage(e)
    }
  }

  async function clearNotes() {
    if (clientId === null) return
    try {
      await clientClearNotes(clientId)
      notes = []
    } catch (e) {
      error = errorMessage(e)
    }
  }

  async function remove() {
    if (clientId === null) return
    try {
      await clientDelete(clientId)
      onBack()
    } catch (e) {
      error = errorMessage(e)
    }
  }

  const sectionsNav = [
    ['sec-audit', 'Audit'], ['sec-identity', 'Identité'], ['sec-situation', 'Situation et études'], ['sec-spouse', 'Époux'],
    ['sec-languages', 'Langues'], ['sec-work', 'Expérience'], ['sec-other', 'Autres facteurs'],
    ['sec-eligibility', 'Admissibilité'],
  ] as const
  const num = (s: string) => (s.trim() === '' ? null : Number(s))
</script>

<PageHeader
  title={clientId === null ? 'Nouveau client' : identity.full_name || 'Client'}
  crumbs={[{ label: 'Clients', go: onBack }]}
  subtitle="Les champs inconnus peuvent rester vides : l’évaluation indique ce qui manque."
>
  {#snippet actions()}
    {#if clientId !== null}<button onclick={() => clientId !== null && onAssess(clientId)}><Icon name="scale" />Voir l’évaluation</button>{/if}
  {/snippet}
</PageHeader>

<ImportNotes {notes} onClear={clearNotes} />

<div class="layout">
<nav class="index" aria-label="Sections du formulaire">
  {#each sectionsNav as [anchor, label] (anchor)}
    {#if (anchor !== 'sec-spouse' || a.spouse) && (anchor !== 'sec-audit' || audit)}<a href={'#' + anchor}>{label}</a>{/if}
  {/each}
</nav>

<form novalidate onsubmit={(e) => (e.preventDefault(), save())}>

  {#if audit}
    <fieldset id="sec-audit">
      <legend>Audit <span class="hint">(document Word généré par « Générer les audits »)</span></legend>
      <div class="row">
        <label>
          CNP du candidat{audit.job ? ` — ${audit.job}` : ''}
          <input inputmode="numeric" maxlength="5" placeholder={audit.suggested_noc ?? '5 chiffres'}
            bind:value={() => audit!.lawyer.noc ?? '', (v) => (audit!.lawyer.noc = v.trim() || null)} />
        </label>
        <label class="check"><input type="checkbox" bind:checked={audit.lawyer.noc_validated} /> Code validé</label>
      </div>
      {#if audit.married}
        <div class="row">
          <label>
            CNP du conjoint{audit.spouse_job ? ` — ${audit.spouse_job}` : ''}
            <input inputmode="numeric" maxlength="5" placeholder={audit.suggested_spouse_noc ?? '5 chiffres'}
              bind:value={() => audit!.lawyer.spouse_noc ?? '', (v) => (audit!.lawyer.spouse_noc = v.trim() || null)} />
          </label>
          <label class="check"><input type="checkbox" bind:checked={audit.lawyer.spouse_noc_validated} /> Code validé</label>
        </div>
      {/if}
      <label>
        Métier réglementé : « Votre profession … »
        <input list="regulated-options" placeholder="à vérifier (surligné dans l’audit)"
          bind:value={() => audit!.lawyer.regulated ?? '', (v) => (audit!.lawyer.regulated = v.trim() || null)} />
        <datalist id="regulated-options">{#each REGULATED as o (o)}<option value={o}></option>{/each}</datalist>
      </label>
      <label>
        Paragraphe libre pour le bilan
        <textarea rows="3" bind:value={() => audit!.lawyer.note ?? '', (v) => (audit!.lawyer.note = v.trim() ? v : null)}></textarea>
      </label>
      <p class="hint">Sans code validé, l’audit garde le code du client (ou un blanc) surligné en jaune. Le FEER découle du 2ᵉ chiffre du code.</p>
    </fieldset>
  {/if}

  <fieldset id="sec-identity">
    <legend>Identité <span class="hint">(jamais envoyée à Claude)</span></legend>
    <div class="row">
      <label>Nom complet <input bind:value={identity.full_name} /></label>
      <label>N° de dossier <input bind:value={() => identity.file_reference ?? '', (v) => (identity.file_reference = v || null)} /></label>
    </div>
    <div class="row">
      <label>Courriel <input type="email" bind:value={() => identity.email ?? '', (v) => (identity.email = v || null)} /></label>
      <label>Téléphone <input bind:value={() => identity.phone ?? '', (v) => (identity.phone = v || null)} /></label>
    </div>
  </fieldset>

  <fieldset id="sec-situation">
    <legend>Situation et études</legend>
    <div class="row">
      <label>État matrimonial
        <select bind:value={a.marital_status}>
          {#each Object.entries(MARITAL) as [k, l] (k)}<option value={k}>{l}</option>{/each}
        </select>
      </label>
      <label>Âge <input type="number" min="1" max="120" bind:value={a.age} /></label>
    </div>
    <label>Plus haut niveau de scolarité
      <select bind:value={a.education}>
        {#each Object.entries(EDUCATION) as [k, l] (k)}<option value={k}>{l}</option>{/each}
      </select>
    </label>
    <label>Titre canadien
      <select bind:value={() => a.canadian_education ?? '', (v) => (a.canadian_education = (v || null) as CanadianEducation | null)}>
        <option value="">Aucun</option>
        {#each Object.entries(CDN_EDUCATION) as [k, l] (k)}<option value={k}>{l}</option>{/each}
      </select>
    </label>
  </fieldset>

  {#if a.spouse}
    <fieldset id="sec-spouse">
      <legend>Époux ou conjoint de fait</legend>
      <label class="check"><input type="checkbox" bind:checked={a.spouse.is_canadian_citizen_or_pr} /> Citoyen(ne) canadien(ne) ou résident(e) permanent(e)</label>
      <label class="check"><input type="checkbox" bind:checked={a.spouse.accompanying} /> Accompagnera le (la) demandeur(se) au Canada</label>
      <div class="row">
        <label>Scolarité
          <select bind:value={() => a.spouse!.education ?? 'secondary', (v) => (a.spouse!.education = v as EducationLevel)}>
            {#each Object.entries(EDUCATION) as [k, l] (k)}<option value={k}>{l}</option>{/each}
          </select>
        </label>
        <label>Années de travail au Canada <input type="number" min="0" max="10" bind:value={a.spouse.canadian_work_years} /></label>
      </div>
      {#if a.spouse.language}
        <LanguageFields bind:value={a.spouse.language} title="Test de langue de l’époux" />
        <button type="button" class="link" onclick={() => (a.spouse!.language = null)}>Retirer le test de l’époux</button>
      {:else}
        <button type="button" class="link" onclick={() => (a.spouse!.language = newLanguageTest())}>+ Test de langue de l’époux</button>
      {/if}
    </fieldset>
  {/if}

  <fieldset id="sec-languages">
    <legend>Langues officielles</legend>
    {#if a.first_language}
      <LanguageFields bind:value={a.first_language} title="Première langue officielle" />
    {:else}
      <p class="error">Aucun test : non admissible à Entrée express.</p>
      <button type="button" class="link" onclick={() => (a.first_language = newLanguageTest())}>+ Ajouter le test</button>
    {/if}
    {#if a.second_language}
      <LanguageFields bind:value={a.second_language} title="Deuxième langue officielle" only={a.first_language && french(a.first_language.test) ? 'english' : 'french'} />
      <button type="button" class="link" onclick={() => (a.second_language = null)}>Retirer la deuxième langue</button>
    {:else}
      <button type="button" class="link" onclick={() => (a.second_language = newLanguageTest(a.first_language && french(a.first_language.test) ? 'ielts_general' : 'tef_canada'))}>+ Deuxième langue officielle</button>
    {/if}
  </fieldset>

  <fieldset id="sec-work">
    <legend>Expérience de travail (pointage SCG)</legend>
    <div class="row">
      <label>Années au Canada (10 dernières années) <input type="number" min="0" max="40" bind:value={a.canadian_work_years} /></label>
      <label>Années à l’étranger <input type="number" min="0" max="40" bind:value={a.foreign_work_years} /></label>
    </div>
    <p class="hint">Dès le 2026-11-03 : détail par CNP (mois équivalents temps plein), pour les professions à salaire élevé.</p>
    {#each a.canadian_work_by_noc as w, i (i)}
      <div class="row">
        <label>CNP <input maxlength="5" bind:value={w.noc} /></label>
        <label>Mois <input type="number" min="0" bind:value={w.months} /></label>
        <button type="button" class="link" onclick={() => a.canadian_work_by_noc.splice(i, 1)}>Retirer</button>
      </div>
    {/each}
    <button type="button" class="link" onclick={() => a.canadian_work_by_noc.push({ noc: '', months: 12 })}>+ Emploi au Canada (CNP)</button>
  </fieldset>

  <fieldset id="sec-other">
    <legend>Autres facteurs</legend>
    <label class="check"><input type="checkbox" bind:checked={a.certificate_of_qualification} /> Certificat de compétence (province/territoire)</label>
    <label class="check"><input type="checkbox" bind:checked={a.provincial_nomination} /> Désignation d’une province ou d’un territoire</label>
    <label class="check"><input type="checkbox" bind:checked={a.sibling_in_canada} /> Frère ou sœur au Canada (18 ans et plus, citoyen ou RP)</label>
    <label class="check">
      <input type="checkbox" checked={a.job_offer !== null} onchange={(e) => (a.job_offer = (e.currentTarget as HTMLInputElement).checked ? { noc: '' } : null)} />
      Offre d’emploi valide
    </label>
    {#if a.job_offer}<label>CNP de l’offre <input maxlength="5" bind:value={a.job_offer.noc} /></label>{/if}
  </fieldset>

  <fieldset id="sec-eligibility">
    <legend>Admissibilité aux programmes</legend>
    <p class="hint">Antécédents de travail détaillés (TQF, CEC, TMS) : chaque emploi compte au maximum 30 h/semaine.</p>
    {#each a.eligibility.work_history as w, i (i)}
      <div class="card">
        <div class="row">
          <label>CNP <input maxlength="5" bind:value={w.noc} /></label>
          <label>Début <input type="date" bind:value={w.start} /></label>
          <label>Fin <input type="date" bind:value={w.end} /></label>
          <label>h/semaine <input type="number" min="1" max="120" bind:value={w.hours_per_week} /></label>
        </div>
        <div class="row">
          <label class="check"><input type="checkbox" bind:checked={w.in_canada} /> Au Canada</label>
          <label class="check"><input type="checkbox" bind:checked={w.paid} /> Rémunéré</label>
          <label class="check"><input type="checkbox" bind:checked={w.self_employed} /> Travailleur autonome</label>
          <label class="check"><input type="checkbox" bind:checked={w.full_time_student} /> Étudiant à temps plein</label>
          <label class="check"><input type="checkbox" bind:checked={w.authorized} /> Autorisé à travailler</label>
        </div>
        <TriState bind:value={w.qualified_in_trade} label="Qualifié pour exercer le métier (TMS)" />
        <button type="button" class="link" onclick={() => a.eligibility.work_history.splice(i, 1)}>Retirer cet emploi</button>
      </div>
    {/each}
    <button type="button" class="link" onclick={() => a.eligibility.work_history.push({ noc: '', in_canada: true, start: '', end: '', hours_per_week: 37, paid: true, self_employed: false, full_time_student: false, authorized: true, qualified_in_trade: null })}>+ Emploi</button>

    <label>Titre de compétence
      <select bind:value={() => a.eligibility.credential ?? '', (v) => (a.eligibility.credential = (v || null) as Credential | null)}>
        <option value="">Inconnu</option>
        {#each Object.entries(CREDENTIAL) as [k, l] (k)}<option value={k}>{l}</option>{/each}
      </select>
    </label>
    <TriState bind:value={a.eligibility.canadian_study_two_years} label="Deux ans d’études à temps plein au Canada" />
    {#if a.spouse}
      <TriState bind:value={a.eligibility.spouse_canadian_study_two_years} label="Époux : deux ans d’études au Canada" />
      <TriState bind:value={a.eligibility.spouse_canadian_work_one_year} label="Époux : un an de travail à temps plein au Canada" />
    {/if}
    <TriState bind:value={a.eligibility.relative_in_canada} label="Membre de la famille au Canada (parent, enfant, frère/sœur, oncle/tante…)" />
    <TriState bind:value={a.eligibility.fsw_arranged_employment} label="Emploi réservé (TQF, situations 1 à 4)" />
    <TriState bind:value={a.eligibility.authorized_to_work_in_canada} label="Actuellement autorisé à travailler au Canada" />
    <TriState bind:value={a.eligibility.intends_to_live_in_quebec} label="Prévoit vivre au Québec" />
    <div class="row">
      <label>Fonds d’établissement ($ CA) <input inputmode="numeric" value={a.eligibility.settlement_funds_cad ?? ''} onchange={(e) => (a.eligibility.settlement_funds_cad = num((e.currentTarget as HTMLInputElement).value))} /></label>
      <label>Taille de la famille <input type="number" min="1" value={a.eligibility.family_size ?? ''} onchange={(e) => (a.eligibility.family_size = num((e.currentTarget as HTMLInputElement).value))} /></label>
    </div>
  </fieldset>

  <div class="savebar">
    {#if problems.length}
      <div class="callout warn problems">
        <strong>Enregistré comme brouillon. À corriger avant l’évaluation :</strong>
        <ul>{#each problems as p (p)}<li>{p}</li>{/each}</ul>
      </div>
    {/if}
    {#if error}<p class="callout err">{error}</p>{/if}
    <div class="bar-row">
      <span class="state">
        {#if dirty}<span class="badge warn">Modifications non enregistrées</span>
        {:else if saved}<span class="badge ok"><Icon name="check" size={12} />Enregistré</span>{/if}
      </span>
      <div class="actions">
        {#if clientId !== null}
          {#if confirmDelete}
            <span class="hint">Supprimer définitivement ce dossier ?</span>
            <button type="button" class="danger" onclick={remove}><Icon name="trash" />Supprimer</button>
            <button type="button" class="ghost" onclick={() => (confirmDelete = false)}>Annuler</button>
          {:else}
            <button type="button" class="ghost" onclick={() => (confirmDelete = true)}><Icon name="trash" />Supprimer…</button>
          {/if}
        {/if}
        <button type="submit">Enregistrer</button>
        <button type="button" class="primary" onclick={() => save(onAssess)}><Icon name="scale" />Enregistrer et évaluer</button>
      </div>
    </div>
  </div>
</form>
</div>

{#if toast}<div class="toast" role="status">{toast}</div>{/if}

<style>
  .layout {
    display: grid;
    grid-template-columns: 11rem 1fr;
    gap: 1.5rem;
    align-items: start;
  }
  .index {
    position: sticky;
    top: 0;
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    padding-top: 0.3rem;
  }
  .index a {
    color: var(--muted);
    text-decoration: none;
    font-size: 0.88rem;
    font-weight: 550;
    padding: 0.35rem 0.6rem;
    border-left: 2px solid var(--border);
  }
  .index a:hover {
    color: var(--accent);
    border-left-color: var(--accent);
  }
  form :global(fieldset) {
    scroll-margin-top: 1rem;
  }
  .savebar {
    position: sticky;
    bottom: -3rem;
    margin: 1rem -0.5rem 0;
    padding: 0.8rem 0.5rem 1rem;
    background: linear-gradient(to top, var(--bg) 75%, transparent);
  }
  .bar-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 1rem;
    flex-wrap: wrap;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    box-shadow: var(--shadow);
    padding: 0.65rem 0.9rem;
  }
  .problems ul {
    margin: 0.4rem 0 0;
    padding-left: 1.2rem;
  }
  @media (max-width: 900px) {
    .layout {
      grid-template-columns: 1fr;
    }
    .index {
      display: none;
    }
  }
</style>
