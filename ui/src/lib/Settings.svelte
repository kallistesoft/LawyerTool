<script lang="ts">
  import { onMount } from 'svelte'
  import { errorMessage, settingsClear, settingsGet, settingsPick, type Folder, type FolderStatus, type Settings } from './api'

  let settings: Settings | null = $state(null)
  let error: string | null = $state(null)
  let busy = $state(false)

  async function act(f: () => Promise<Settings>) {
    error = null
    busy = true
    try {
      settings = await f()
    } catch (e) {
      error = errorMessage(e)
    } finally {
      busy = false
    }
  }

  onMount(() => act(settingsGet))

  const folders: { which: Folder; title: string; help: string }[] = [
    {
      which: 'input',
      title: 'Dossier des résultats de formulaires',
      help: 'Les exports de formulaires (.xlsx, .csv) à analyser. L’application lit ce dossier uniquement quand vous cliquez « Analyser », sans ses sous-dossiers, et ne modifie jamais les fichiers.',
    },
    {
      which: 'output',
      title: 'Dossier de sortie des évaluations',
      help: 'Les rapports PDF / Word révisés y sont enregistrés directement (jamais écrasés). Ces fichiers ne sont pas chiffrés : choisissez un dossier protégé.',
    },
  ]

  const statusOf = (which: Folder): FolderStatus | null =>
    settings ? (which === 'input' ? settings.input_dir : settings.output_dir) : null

  function problem(which: Folder, s: FolderStatus): string | null {
    if (!s.exists) return 'Dossier introuvable.'
    if (which === 'output' && !s.writable) return 'Écriture impossible dans ce dossier.'
    return null
  }
</script>

<section>
  <h2>Paramètres</h2>
  {#if error}<p class="error">{error}</p>{/if}
  {#each folders as f (f.which)}
    {@const s = statusOf(f.which)}
    <div class="folder">
      <h3>{f.title}</h3>
      <p class="hint">{f.help}</p>
      {#if s}
        <p><code>{s.path}</code></p>
        {#if problem(f.which, s)}<p class="error">{problem(f.which, s)}</p>{/if}
      {:else}
        <p class="hint">
          {f.which === 'input' ? 'Aucun dossier choisi.' : 'Aucun dossier choisi : l’export demande l’emplacement à chaque fois.'}
        </p>
      {/if}
      <div class="row">
        <button disabled={busy} onclick={() => act(() => settingsPick(f.which))}>Choisir…</button>
        {#if s}<button class="link" disabled={busy} onclick={() => act(() => settingsClear(f.which))}>Retirer</button>{/if}
      </div>
    </div>
  {/each}
</section>

<style>
  .folder {
    margin-bottom: 1.5rem;
  }
  code {
    word-break: break-all;
  }
</style>
