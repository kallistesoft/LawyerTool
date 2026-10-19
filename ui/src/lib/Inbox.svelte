<script lang="ts">
  import { onMount } from 'svelte'
  import { errorMessage, inboxImport, inboxList, type ImportResult, type InboxFile } from './api'

  let { onOpen, onSettings }: { onOpen: (id: number) => void; onSettings: () => void } = $props()
  let files: InboxFile[] = $state([])
  let results: ImportResult[] = $state([])
  let error: string | null = $state(null)
  let noFolder = $state(false)
  let busy = $state(false)
  let pending = $derived(files.filter((f) => f.status === 'new' || f.status === 'failed'))

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
    try {
      results = await inboxImport(pending.map((f) => f.name))
      await refresh()
      const created = results.filter((r) => r.ok && r.client_id !== null)
      if (created.length === 1 && created[0].client_id !== null) onOpen(created[0].client_id)
    } catch (e) {
      error = errorMessage(e)
    } finally {
      busy = false
    }
  }

  onMount(refresh)

  const label: Record<InboxFile['status'], string> = {
    new: 'Nouveau',
    imported: 'Importé',
    failed: 'Échec',
    too_large: 'Trop volumineux',
  }
  const kb = (n: number) => `${Math.max(1, Math.round(n / 1024))} Ko`
</script>

<section class="inbox">
  <div class="row between">
    <h3>Formulaires reçus</h3>
    {#if !noFolder}
      <div class="row">
        <button class="link" onclick={refresh} disabled={busy}>Actualiser</button>
        <button onclick={analyse} disabled={busy || pending.length === 0}>Analyser les nouveaux formulaires</button>
      </div>
    {/if}
  </div>
  {#if noFolder}
    <p class="hint">
      Aucun dossier de formulaires. <button class="link" onclick={onSettings}>Choisir dans Paramètres</button>
    </p>
  {:else if error}
    <p class="error">{error}</p>
  {:else if files.length === 0}
    <p class="hint">Aucun fichier .xlsx ou .csv dans le dossier.</p>
  {:else}
    <table>
      <thead><tr><th>Fichier</th><th>Taille</th><th>Modifié</th><th>État</th></tr></thead>
      <tbody>
        {#each files as f (f.name)}
          <tr>
            <td>{f.name}</td>
            <td>{kb(f.size)}</td>
            <td>{f.modified?.slice(0, 10) ?? ''}</td>
            <td>
              {#if f.status === 'imported' && f.client_id !== null}
                <button class="link" onclick={() => f.client_id !== null && onOpen(f.client_id)}>{label[f.status]}</button>
              {:else}
                {label[f.status]}
              {/if}
              {#if f.message}<div class="hint">{f.message}</div>{/if}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
  {#if results.length > 0}
    <ul>
      {#each results as r (r.name)}
        <li class={r.ok ? '' : 'error'}>{r.name} : {r.message}</li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .inbox {
    margin-bottom: 1.5rem;
  }
</style>
