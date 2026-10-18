<script lang="ts">
  import { onMount } from 'svelte'
  import {
    errorMessage, onWatchUpdated, watchApprove, watchCheckNow, watchReject, watchStatus, type WatchStatus,
  } from './api'

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

  const fmt = (t: string) => t.replace('T', ' ').replace('Z', ' UTC')
  const pending = $derived(status?.pending?.[1] ?? null)
</script>

<section>
  <h2>Veille des règles officielles</h2>
  <p class="hint">
    Pages d’IRCC relues chaque jour (coffre déverrouillé). Aucune règle ne change sans votre approbation ; la grille
    approuvée reste en vigueur en cas d’échec.
  </p>
  {#if status}
    {#if status.stale}
      <p class="warn"><strong>Règles non vérifiées depuis plus de 7 jours.</strong> Lancez une vérification.</p>
    {/if}
    <p>
      Dernière vérification : {status.last_run ? `${fmt(status.last_run.at)} — ${status.last_run.ok ? 'réussie' : 'incomplète'} (${status.last_run.summary})` : 'jamais'}
      {#if status.last_ok && status.last_run && !status.last_run.ok}<br />Dernière réussie : {fmt(status.last_ok.at)}{/if}
    </p>
    <button disabled={busy || status.running} onclick={() => act(watchCheckNow)}>
      {busy || status.running ? 'Vérification…' : 'Vérifier maintenant'}
    </button>
    {#if error}<p class="error">{error}</p>{/if}

    {#if pending}
      <h3>À examiner (depuis {fmt(status.pending![0])})</h3>
      {#each pending.alerts as a (a)}<p class="warn">{a}</p>{/each}
      {#each pending.part_errors as e (e)}
        <p class="warn">Page non lue, la grille approuvée reste en vigueur pour cette partie : {e}</p>
      {/each}
      {#if pending.changes.length}
        <p><strong>{pending.changes.length} valeur(s) modifiée(s)</strong> par rapport à la grille {pending.base_grid} :</p>
        <table>
          <thead><tr><th>Valeur</th><th>Avant</th><th>Après</th><th>Ligne officielle</th></tr></thead>
          <tbody>
            {#each pending.changes as c (c.path)}
              <tr><td><code>{c.path}</code></td><td>{c.before ?? '—'}</td><td><strong>{c.after ?? '—'}</strong></td><td class="cites">{c.source_text ?? ''}</td></tr>
            {/each}
          </tbody>
        </table>
      {/if}
      {#each pending.prose as p (p.source)}
        <details open>
          <summary>Texte modifié : {p.source}</summary>
          {#each p.changes as c, i (i)}
            <p class={'added' in c ? 'added' : 'removed'}>{'added' in c ? `+ ${c.added}` : `− ${c.removed}`}</p>
          {/each}
        </details>
      {/each}
      <label>Note d’approbation (facultative) <input bind:value={note} /></label>
      <div class="row">
        <button disabled={busy} onclick={() => act(async () => (approved = await watchApprove(note || null)))}>
          {pending.changes.length ? 'Approuver les modifications' : 'Marquer comme examiné'}
        </button>
        {#if pending.changes.length}<button class="link" disabled={busy} onclick={() => act(watchReject)}>Rejeter</button>{/if}
      </div>
    {:else}
      <p>Aucune modification en attente.</p>
    {/if}
    {#if approved?.length}<p>Nouvelle(s) grille(s) approuvée(s) : {approved.join(', ')}</p>{/if}

    <h3>Grilles en vigueur</h3>
    <ul>{#each status.grids as [id, from] (id)}<li><code>{id}</code> — à partir du {from}</li>{/each}</ul>

    <h3>Derniers tirages (rondes d’invitations)</h3>
    {#if status.draws.length}
      <table>
        <thead><tr><th>N°</th><th>Date</th><th>Type</th><th>Invitations</th><th>Note minimale</th></tr></thead>
        <tbody>
          {#each status.draws as d (d.number)}
            <tr><td>{d.number}</td><td>{d.date}</td><td>{d.name}</td><td>{d.invitations.toLocaleString('fr-CA')}</td><td><strong>{d.min_crs}</strong></td></tr>
          {/each}
        </tbody>
      </table>
    {:else}
      <p class="hint">Aucun tirage enregistré : lancez une vérification.</p>
    {/if}
  {/if}
</section>

<style>
  .cites {
    font-size: 0.72rem;
    opacity: 0.75;
  }
  .added {
    color: #1e7d32;
  }
  .removed {
    color: #b00020;
    text-decoration: line-through;
  }
</style>
