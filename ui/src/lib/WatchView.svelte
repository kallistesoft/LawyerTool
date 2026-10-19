<script lang="ts">
  import { onMount } from 'svelte'
  import {
    errorMessage, onWatchUpdated, watchApprove, watchCheckNow, watchReject, watchStatus, type WatchStatus,
  } from './api'
  import Icon from './Icon.svelte'
  import PageHeader from './PageHeader.svelte'

  let status = $state<WatchStatus | null>(null)
  let error: string | null = $state(null)
  let busy = $state(false)
  let note = $state('')
  let approved: string[] | null = $state(null)

  const refresh = () => watchStatus().then((s) => (status = s)).catch((e) => (error = errorMessage(e)))

  onMount(() => {
    refresh()
    const un = onWatchUpdated(refresh)
    return () => {
      un.then((f) => f())
    }
  })

  async function act(f: () => Promise<unknown>) {
    busy = true
    error = null
    try {
      await f()
    } catch (e) {
      error = errorMessage(e)
    } finally {
      busy = false
      await refresh()
    }
  }

  const fmt = (t: string) =>
    new Date(t).toLocaleString('fr-CA', { day: 'numeric', month: 'long', year: 'numeric', hour: '2-digit', minute: '2-digit' })
  const pending = $derived(status?.pending?.[1] ?? null)
</script>

<PageHeader title="Veille des règles" subtitle="Pages officielles d’IRCC relues chaque jour. Aucune règle ne change sans votre approbation ; en cas d’échec, la grille approuvée reste en vigueur.">
  {#snippet actions()}
    <button class="primary" disabled={busy || !status || status.running} onclick={() => act(watchCheckNow)}>
      <Icon name="refresh" />{busy || status?.running ? 'Vérification…' : 'Vérifier maintenant'}
    </button>
  {/snippet}
</PageHeader>

{#if error}<p class="callout err">{error}</p>{/if}

{#if status}
  <div class="grid-3 top">
    <div class="card stat">
      <div class="label">Dernière vérification</div>
      {#if status.last_run}
        <div class="value">{fmt(status.last_run.at)}</div>
        <span class="badge {status.last_run.ok ? 'ok' : 'warn'}">{status.last_run.ok ? 'Réussie' : 'Incomplète'}</span>
        <p class="hint">{status.last_run.summary}</p>
        {#if status.last_ok && !status.last_run.ok}<p class="hint">Dernière réussie : {fmt(status.last_ok.at)}</p>{/if}
      {:else}
        <div class="value">Jamais</div>
      {/if}
    </div>
    <div class="card stat">
      <div class="label">État des règles</div>
      {#if pending}
        <div class="value warn-text">À examiner</div>
        <span class="badge warn">Depuis le {fmt(status.pending![0])}</span>
      {:else if status.stale}
        <div class="value warn-text">À vérifier</div>
        <span class="badge warn">Plus de 7 jours sans vérification</span>
      {:else}
        <div class="value ok-text">À jour</div>
        <span class="badge ok"><Icon name="check" size={12} />Aucune modification en attente</span>
      {/if}
    </div>
    <div class="card stat">
      <div class="label">Grilles en vigueur</div>
      <ul class="grids">{#each status.grids as [id, from] (id)}<li><code>{id}</code> <span class="hint">à partir du {from}</span></li>{/each}</ul>
    </div>
  </div>

  {#if pending}
    <div class="card pending">
      <div class="card-title">
        <h3><Icon name="alert" /> Modifications à examiner</h3>
        <span class="hint">par rapport à la grille {pending.base_grid}</span>
      </div>
      {#each pending.alerts as a (a)}<p class="callout warn">{a}</p>{/each}
      {#each pending.part_errors as e (e)}
        <p class="callout warn">Page non lue, la grille approuvée reste en vigueur pour cette partie : {e}</p>
      {/each}
      {#if pending.changes.length}
        <p><strong>{pending.changes.length} valeur{pending.changes.length > 1 ? 's' : ''} modifiée{pending.changes.length > 1 ? 's' : ''}</strong></p>
        <div class="table-wrap">
          <table>
            <thead><tr><th>Valeur</th><th>Avant</th><th></th><th>Après</th><th>Ligne officielle</th></tr></thead>
            <tbody>
              {#each pending.changes as c (c.path)}
                <tr>
                  <td><code>{c.path}</code></td>
                  <td><span class="old">{c.before ?? '—'}</span></td>
                  <td class="hint">→</td>
                  <td><span class="new">{c.after ?? '—'}</span></td>
                  <td class="cites">{c.source_text ?? ''}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
      {#each pending.prose as p (p.source)}
        <details open class="prose">
          <summary><strong>Texte modifié</strong> · {p.source}</summary>
          {#each p.changes as c, i (i)}
            <p class={'added' in c ? 'added' : 'removed'}>{'added' in c ? `+ ${c.added}` : `− ${c.removed}`}</p>
          {/each}
        </details>
      {/each}
      <label>Note d’approbation (facultative) <input bind:value={note} placeholder="Ex. vérifié avec l’annonce d’IRCC du …" /></label>
      <div class="actions">
        <button class="primary" disabled={busy} onclick={() => act(async () => (approved = await watchApprove(note || null)))}>
          <Icon name="check" />{pending.changes.length ? 'Approuver les modifications' : 'Marquer comme examiné'}
        </button>
        {#if pending.changes.length}<button class="ghost" disabled={busy} onclick={() => act(watchReject)}><Icon name="x" />Rejeter</button>{/if}
      </div>
    </div>
  {/if}
  {#if approved?.length}<p class="callout ok">Nouvelle(s) grille(s) approuvée(s) : {approved.join(', ')}</p>{/if}

  <div class="card">
    <div class="card-title"><h3>Derniers tirages (rondes d’invitations)</h3></div>
    {#if status.draws.length}
      <div class="table-wrap">
        <table>
          <thead><tr><th>N°</th><th>Date</th><th>Type</th><th class="num">Invitations</th><th class="num">Note minimale</th></tr></thead>
          <tbody>
            {#each status.draws as d (d.number)}
              <tr><td class="hint">{d.number}</td><td>{d.date}</td><td>{d.name}</td><td class="num">{d.invitations.toLocaleString('fr-CA')}</td><td class="num"><strong>{d.min_crs}</strong></td></tr>
            {/each}
          </tbody>
        </table>
      </div>
    {:else}
      <p class="hint">Aucun tirage enregistré : lancez une vérification.</p>
    {/if}
  </div>
{/if}

<style>
  .top {
    margin-bottom: 1rem;
  }
  .top > .card {
    margin: 0;
  }
  .label {
    font-size: 0.75rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--muted);
    margin-bottom: 0.4rem;
  }
  .value {
    font-size: 1.2rem;
    font-weight: 700;
    margin-bottom: 0.35rem;
  }
  .ok-text { color: var(--ok); }
  .warn-text { color: var(--warn); }
  .grids {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .grids li {
    padding: 0.2rem 0;
  }
  .pending {
    border-color: color-mix(in srgb, var(--warn) 45%, var(--border));
  }
  .pending h3 {
    display: flex;
    gap: 0.4rem;
    align-items: center;
    color: var(--warn);
  }
  .old {
    color: var(--err);
    text-decoration: line-through;
  }
  .new {
    color: var(--ok);
    font-weight: 700;
  }
  .cites {
    font-size: 0.76rem;
    color: var(--muted);
  }
  .prose {
    margin: 0.8rem 0;
  }
  .added {
    color: var(--ok);
  }
  .removed {
    color: var(--err);
    text-decoration: line-through;
  }
</style>
