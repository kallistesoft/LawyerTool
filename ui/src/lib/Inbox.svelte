<script lang="ts">
  import { onMount } from 'svelte'
  import { errorMessage, inboxImport, inboxList, type ImportResult, type InboxFile } from './api'
  import Icon from './Icon.svelte'

  let { onOpen, onSettings, onCount, onImported }: {
    onOpen: (id: number) => void
    onSettings: () => void
    onCount: (n: number) => void
    onImported: () => void
  } = $props()
  let files: InboxFile[] = $state([])
  let results: ImportResult[] = $state([])
  let error: string | null = $state(null)
  let noFolder = $state(false)
  let busy = $state(false)
  let open = $state(false)
  let pending = $derived(files.filter((f) => f.status === 'new' || f.status === 'failed'))
  $effect(() => onCount(files.filter((f) => f.status === 'new').length))

  async function refresh() {
    error = null
    noFolder = false
    try {
      files = await inboxList()
    } catch (e) {
      if (typeof e === 'object' && e !== null && (e as { code?: string }).code === 'no_input_dir') noFolder = true
      else error = errorMessage(e)
    }
  }

  async function analyse() {
    busy = true
    error = null
    open = true
    try {
      results = await inboxImport(pending.map((f) => f.name))
      await refresh()
      onImported()
      const created = results.filter((r) => r.ok && r.client_id !== null)
      if (created.length === 1 && created[0].client_id !== null) onOpen(created[0].client_id)
    } catch (e) {
      error = errorMessage(e)
    } finally {
      busy = false
    }
  }

  onMount(refresh)

  const label: Record<InboxFile['status'], [string, string]> = {
    new: ['Nouveau', 'info'],
    imported: ['Importé', 'ok'],
    failed: ['Échec', 'err'],
    too_large: ['Trop volumineux', 'err'],
  }
  const kb = (n: number) => `${Math.max(1, Math.round(n / 1024))} Ko`
  const newCount = $derived(files.filter((f) => f.status === 'new').length)
</script>

<div class="card inbox">
  <div class="head">
    <button class="ghost toggle" onclick={() => (open = !open)} disabled={noFolder} aria-expanded={open}>
      <Icon name="inbox" size={18} />
      <strong>Formulaires reçus</strong>
      {#if newCount}<span class="badge info">{newCount} nouveau{newCount > 1 ? 'x' : ''}</span>{/if}
      {#if !noFolder}<span class="chev">{open ? '▾' : '▸'}</span>{/if}
    </button>
    {#if noFolder}
      <span class="hint">Aucun dossier de formulaires. <button class="link" onclick={onSettings}>Le choisir dans Paramètres</button></span>
    {:else}
      <div class="actions">
        <button class="ghost small" onclick={refresh} disabled={busy} title="Actualiser"><Icon name="refresh" size={14} /></button>
        <button class="primary small" onclick={analyse} disabled={busy || pending.length === 0}>
          {busy ? 'Analyse…' : `Analyser${pending.length ? ` (${pending.length})` : ''}`}
        </button>
      </div>
    {/if}
  </div>
  {#if error}<p class="error">{error}</p>{/if}
  {#if open && !noFolder}
    {#if files.length === 0}
      <p class="hint">Aucun fichier .xlsx ou .csv dans le dossier.</p>
    {:else}
      <div class="table-wrap">
        <table>
          <thead><tr><th>Fichier</th><th>Taille</th><th>Modifié</th><th>État</th></tr></thead>
          <tbody>
            {#each files as f (f.name)}
              <tr>
                <td>{f.name}</td>
                <td class="hint num">{kb(f.size)}</td>
                <td class="hint">{f.modified?.slice(0, 10) ?? ''}</td>
                <td>
                  {#if f.status === 'imported' && f.client_id !== null}
                    <button class="link" onclick={() => f.client_id !== null && onOpen(f.client_id)}>
                      <span class="badge ok">Importé · ouvrir</span>
                    </button>
                  {:else}
                    <span class="badge {label[f.status][1]}">{label[f.status][0]}</span>
                  {/if}
                  {#if f.message}<div class="hint">{f.message}</div>{/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
    {#if results.length > 0}
      <ul class="results">
        {#each results as r (r.name)}
          <li class={r.ok ? '' : 'error'}>{r.name} : {r.message}</li>
        {/each}
      </ul>
    {/if}
  {/if}
</div>

<style>
  .inbox {
    padding: 0.6rem 1rem;
  }
  .head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 1rem;
    flex-wrap: wrap;
  }
  .toggle {
    gap: 0.55rem;
    padding: 0.35rem 0.4rem;
  }
  .chev {
    color: var(--muted);
  }
  .results {
    margin: 0.5rem 0 0.2rem;
    padding-left: 1.2rem;
    font-size: 0.88rem;
  }
</style>
