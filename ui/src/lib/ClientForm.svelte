<script lang="ts">
  import { onMount } from 'svelte'
  import { clientDelete, clientGet, clientSave, errorMessage } from './api'
  import LanguageFields from './LanguageFields.svelte'
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
  let error: string | null = $state(null)
  let saved = $state(false)
  let confirmDelete = $state(false)

  onMount(() => {
    clientId = id
    if (id !== null) {
      clientGet(id)
        .then((c) => {
          identity = c.identity
          a = c.profile
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
    try {
      const r = await clientSave(clientId, identity, a)
      clientId = r.id
      problems = r.problems
      saved = true
      onSaved(r.id)
      if (then && r.problems.length === 0) then(r.id)
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

  const num = (s: string) => (s.trim() === '' ? null : Number(s))
</script>

<form onsubmit={(e) => (e.preventDefault(), save())}>
  <div class="row between">
    <h2>{clientId === null ? 'Nouveau client' : identity.full_name || 'Client'}</h2>
    <button type="button" class="link" onclick={onBack}>← Liste des clients</button>
  </div>

  <fieldset>
    <legend>Identité <span class="hint">(jamais envoyée à Claude)</span></legend>
    <div class="row">
      <label>Nom complet <input required bind:value={identity.full_name} /></label>
      <label>N° de dossier <input bind:value={() => identity.file_reference ?? '', (v) => (identity.file_reference = v || null)} /></label>
    </div>
    <div class="row">
      <label>Courriel <input type="email" bind:value={() => identity.email ?? '', (v) => (identity.email = v || null)} /></label>
      <label>Téléphone <input bind:value={() => identity.phone ?? '', (v) => (identity.phone = v || null)} /></label>
    </div>
  </fieldset>

  <fieldset>
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
    <fieldset>
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

  <fieldset>
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

  <fieldset>
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

  <fieldset>
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

  <fieldset>
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

  {#if problems.length}
    <div class="warn">
      <p><strong>Enregistré comme brouillon. À corriger avant l’évaluation :</strong></p>
      <ul>{#each problems as p (p)}<li>{p}</li>{/each}</ul>
    </div>
  {:else if saved}
    <p class="hint">Enregistré.</p>
  {/if}
  {#if error}<p class="error">{error}</p>{/if}

  <div class="row">
    <button type="submit">Enregistrer</button>
    <button type="button" onclick={() => save(onAssess)}>Enregistrer et évaluer</button>
    {#if clientId !== null}
      {#if confirmDelete}
        <button type="button" class="danger" onclick={remove}>Confirmer la suppression</button>
        <button type="button" class="link" onclick={() => (confirmDelete = false)}>Annuler</button>
      {:else}
        <button type="button" class="link" onclick={() => (confirmDelete = true)}>Supprimer…</button>
      {/if}
    {/if}
  </div>
</form>
