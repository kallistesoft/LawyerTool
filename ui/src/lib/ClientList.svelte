<script lang="ts">
  import { onMount } from 'svelte'
  import { clientOverview, errorMessage } from './api'
  import Icon from './Icon.svelte'
  import Inbox from './Inbox.svelte'
  import {
    localToday, PROGRAM, PROGRAM_SHORT, STATUS_LABEL, STATUS_TONE, VERDICT_LABEL, VERDICT_TONE, type ClientOverview,
  } from './model'
  import PageHeader from './PageHeader.svelte'

  let { onOpen, onNew, onSettings }: { onOpen: (id: number) => void; onNew: () => void; onSettings: () => void } = $props()
  let clients: ClientOverview[] = $state([])
  let loaded = $state(false)
  let error: string | null = $state(null)
  let filter = $state('')
  let newForms = $state(0)
  type Key = 'name' | 'crs' | 'updated'
  let sort = $state<{ key: Key; desc: boolean }>({ key: 'updated', desc: true })

  const load = () =>
    clientOverview(localToday())
      .then((c) => ((clients = c), (loaded = true)))
      .catch((e) => ((error = errorMessage(e)), (loaded = true)))
  onMount(load)

  const needsAttention = (c: ClientOverview) => c.reviews > 0 || c.problems > 0 || c.crs === null
  const shown = $derived.by(() => {
    const q = filter.trim().toLowerCase()
    const rows = clients.filter((c) => (c.full_name + ' ' + (c.file_reference ?? '')).toLowerCase().includes(q))
    const val = (c: ClientOverview) =>
      sort.key === 'name' ? c.full_name.toLowerCase() : sort.key === 'crs' ? (c.crs ?? -1) : c.updated_at
    return rows.sort((a, b) => {
      const x = val(a)
      const y = val(b)
      const r = x < y ? -1 : x > y ? 1 : 0
      return sort.desc ? -r : r
    })
  })
  const toggle = (key: Key) => (sort = { key, desc: sort.key === key ? !sort.desc : key !== 'name' })
  const arrow = (key: Key) => (sort.key === key ? (sort.desc ? ' ↓' : ' ↑') : '')
  const eligibleCount = $derived(clients.filter((c) => c.programs.some(([, s]) => s === 'Eligible')).length)
  const attention = $derived(clients.filter(needsAttention).length)
  const date = (t: string) => new Date(t).toLocaleDateString('fr-CA', { day: 'numeric', month: 'short', year: 'numeric' })
</script>

<PageHeader title="Clients" subtitle="Score SCG et admissibilité calculés sur la grille officielle en vigueur aujourd’hui.">
  {#snippet actions()}
    <button class="primary" onclick={onNew}><Icon name="plus" />Nouveau client</button>
  {/snippet}
</PageHeader>

<div class="kpis">
  <div class="card kpi">
    <span class="kpi-label">Clients</span>
    <span class="kpi-value">{clients.length}</span>
  </div>
  <div class="card kpi">
    <span class="kpi-label">Admissibles à au moins un programme</span>
    <span class="kpi-value ok">{eligibleCount}</span>
  </div>
  <div class="card kpi">
    <span class="kpi-label">À vérifier</span>
    <span class="kpi-value" class:attn={attention > 0}>{attention}</span>
  </div>
  <div class="card kpi">
    <span class="kpi-label">Nouveaux formulaires</span>
    <span class="kpi-value" class:info={newForms > 0}>{newForms}</span>
  </div>
</div>

<Inbox {onOpen} {onSettings} onCount={(n) => (newForms = n)} onImported={load} />

<div class="card list">
  <div class="card-title">
    <h3>Dossiers</h3>
    <label class="search">
      <Icon name="search" />
      <input placeholder="Rechercher un nom ou un n° de dossier" bind:value={filter} />
    </label>
  </div>
  {#if error}<p class="error">{error}</p>{/if}
  {#if loaded && clients.length === 0}
    <div class="empty">
      <Icon name="clients" size={40} />
      <h3>Aucun client pour l’instant</h3>
      <p class="hint">Créez un dossier, ou déposez un export de formulaire dans le dossier des formulaires.</p>
      <button class="primary" onclick={onNew}><Icon name="plus" />Nouveau client</button>
    </div>
  {:else if loaded && shown.length === 0}
    <p class="hint empty-search">Aucun client ne correspond à « {filter} ».</p>
  {:else}
    <div class="table-wrap">
      <table>
        <thead>
          <tr>
            <th class="sortable" onclick={() => toggle('name')}>Client{arrow('name')}</th>
            <th class="sortable score-col" onclick={() => toggle('crs')}>Score SCG{arrow('crs')}</th>
            <th>Programmes</th>
            <th>À vérifier</th>
            <th class="sortable" onclick={() => toggle('updated')}>Modifié{arrow('updated')}</th>
          </tr>
        </thead>
        <tbody>
          {#each shown as c (c.id)}
            <tr class="clickable" onclick={() => onOpen(c.id)}>
              <td>
                <div class="name">{c.full_name}</div>
                <div class="hint">{c.file_reference ? `Dossier ${c.file_reference}` : 'Sans n° de dossier'}</div>
              </td>
              <td class="score-col">
                {#if c.crs !== null}
                  <span class="score num">{c.crs}</span>
                {:else if c.crs_estimate !== null}
                  <span class="score num estimate" title="Score estimé d’après le formulaire (aucun test réel)">≈ {c.crs_estimate}</span>
                {:else}
                  <span class="hint" title={c.crs_error ?? ''}>—</span>
                {/if}
              </td>
              <td>
                <div class="progs">
                  {#if c.verdict && c.crs === null}
                    <span class="badge {VERDICT_TONE[c.verdict]}" title="Pré-évaluation d’après le formulaire">Pré-évaluation : {VERDICT_LABEL[c.verdict]}</span>
                  {:else}
                  {#each c.programs as [p, s] (p)}
                    <span class="badge {STATUS_TONE[s]}" title="{PROGRAM[p]} : {STATUS_LABEL[s]}">{PROGRAM_SHORT[p]}</span>
                  {/each}
                  {/if}
                </div>
              </td>
              <td>
                <div class="progs">
                  {#if c.reviews > 0}<span class="badge warn"><Icon name="scale" size={12} />{c.reviews} révision{c.reviews > 1 ? 's' : ''}</span>{/if}
                  {#if c.problems > 0}<span class="badge err"><Icon name="alert" size={12} />Profil incomplet</span>{/if}
                  {#if c.crs === null && c.problems === 0 && c.crs_estimate === null}<span class="badge err" title={c.crs_error ?? ''}>Score impossible</span>{/if}
                  {#if c.crs === null && c.crs_estimate !== null}<span class="badge info" title="Saisir les résultats de test pour le score officiel">Score officiel à établir</span>{/if}
                  {#if !needsAttention(c)}<span class="badge ok"><Icon name="check" size={12} />Prêt</span>{/if}
                </div>
              </td>
              <td class="hint">{date(c.updated_at)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
    <p class="hint legend">
      <span class="badge ok">TQF</span> admissible ·
      <span class="badge warn">CEC</span> renseignements à compléter ·
      <span class="badge err">TMS</span> non admissible
    </p>
  {/if}
</div>

<style>
  .kpis {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(11rem, 1fr));
    gap: 1rem;
    margin-bottom: 1rem;
  }
  .kpi {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    margin: 0;
    padding: 0.9rem 1.1rem;
  }
  .kpi-label {
    font-size: 0.78rem;
    color: var(--muted);
    font-weight: 600;
  }
  .kpi-value {
    font-size: 1.7rem;
    font-weight: 750;
    font-variant-numeric: tabular-nums;
  }
  .kpi-value.ok { color: var(--ok); }
  .kpi-value.attn { color: var(--warn); }
  .kpi-value.info { color: var(--info); }
  .search {
    position: relative;
    margin: 0;
    width: min(22rem, 100%);
    color: var(--muted);
  }
  .search :global(svg) {
    position: absolute;
    left: 0.6rem;
    top: 50%;
    transform: translateY(-50%);
  }
  .search input {
    margin: 0;
    padding-left: 2rem;
  }
  .list {
    padding-bottom: 0.6rem;
  }
  .name {
    font-weight: 650;
  }
  .score-col {
    text-align: right;
    width: 7rem;
  }
  .score {
    font-size: 1.25rem;
    font-weight: 750;
    color: var(--accent);
  }
  .progs {
    display: flex;
    gap: 0.3rem;
    flex-wrap: wrap;
  }
  .empty {
    text-align: center;
    padding: 2.5rem 1rem;
    color: var(--muted);
  }
  .empty h3 {
    margin-top: 0.6rem;
    color: var(--text);
  }
  .empty button {
    margin-top: 0.8rem;
  }
  .empty-search {
    padding: 1rem 0;
  }
  .legend {
    margin-top: 0.6rem;
  }
</style>
