<script lang="ts">
  import { onMount } from 'svelte'
  import Icon from './Icon.svelte'
  import PageHeader from './PageHeader.svelte'
  import {
    devEraseAll, devMode, errorMessage, folderAccess, settingsClear, settingsGet, settingsPick,
    type Folder, type FolderStatus, type Settings,
  } from './api'

  let settings: Settings | null = $state(null)
  let error: string | null = $state(null)
  let busy = $state(false)
  let confirmErase = $state(false)

  async function eraseAll() {
    busy = true
    try {
      await devEraseAll()
    } catch (e) {
      error = errorMessage(e)
      busy = false
    }
  }

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

  const needsPermission = $derived(!!(statusOf('input')?.needs_permission || statusOf('output')?.needs_permission))

  function problem(which: Folder, s: FolderStatus): string | null {
    if (s.needs_permission) return 'Accès à confirmer'
    if (!s.exists) return 'Dossier introuvable.'
    if (which === 'output' && !s.writable) return 'Écriture impossible dans ce dossier.'
    return null
  }
</script>

<PageHeader title="Paramètres" subtitle="Dossiers de travail sur cet ordinateur, mémorisés par ce navigateur." />

{#if error}<p class="callout err">{error}</p>{/if}
{#if needsPermission}
  <div class="callout warn access">
    <span>Le navigateur demande de confirmer l’accès aux dossiers à chaque nouvelle session.</span>
    <button class="primary" disabled={busy} onclick={() => act(folderAccess)}><Icon name="folder" />Autoriser l’accès aux dossiers</button>
  </div>
{/if}

<div class="folders">
  {#each folders as f (f.which)}
    {@const s = statusOf(f.which)}
    {@const issue = s ? problem(f.which, s) : null}
    <div class="card folder">
      <div class="card-title">
        <h3><span class="ico"><Icon name="folder" size={18} /></span>{f.title}</h3>
        {#if !s}<span class="badge">Non défini</span>
        {:else if issue}<span class="badge {s.needs_permission ? 'warn' : 'err'}">{issue}</span>
        {:else}<span class="badge ok"><Icon name="check" size={12} />Prêt</span>{/if}
      </div>
      <p class="hint">{f.help}</p>
      {#if s}
        <div class="path"><code>{s.path}</code></div>
      {:else}
        <p class="hint empty">
          {f.which === 'input' ? 'Aucun dossier choisi.' : 'Aucun dossier choisi : l’export demande l’emplacement à chaque fois.'}
        </p>
      {/if}
      <div class="actions">
        <button class={s ? '' : 'primary'} disabled={busy} onclick={() => act(() => settingsPick(f.which))}><Icon name="folder" />{s ? 'Changer…' : 'Choisir…'}</button>
        {#if s}<button class="ghost" disabled={busy} onclick={() => act(() => settingsClear(f.which))}>Retirer</button>{/if}
      </div>
    </div>
  {/each}
</div>

{#if devMode}
  <div class="card danger-zone">
    <div class="card-title"><h3><span class="ico"><Icon name="trash" size={18} /></span>Version de démonstration</h3></div>
    <p class="hint">
      Efface toutes les données de l’application dans ce navigateur : clients, résultats de la veille, réglages et
      dossiers choisis. Les fichiers de vos dossiers ne sont pas touchés. L’application repart d’un coffre de test vide.
    </p>
    <div class="actions">
      {#if confirmErase}
        <span class="hint">Effacer définitivement toutes les données ?</span>
        <button class="danger" disabled={busy} onclick={eraseAll}><Icon name="trash" />{busy ? 'Effacement…' : 'Tout effacer'}</button>
        <button class="ghost" disabled={busy} onclick={() => (confirmErase = false)}>Annuler</button>
      {:else}
        <button class="ghost" onclick={() => (confirmErase = true)}><Icon name="trash" />Effacer toutes les données…</button>
      {/if}
    </div>
  </div>
{/if}

<style>
  .access {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 1rem;
    flex-wrap: wrap;
  }
  .folders {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(22rem, 1fr));
    gap: 1rem;
  }
  .folder {
    margin: 0;
  }
  .danger-zone {
    margin-top: 1rem;
  }
  h3 {
    display: flex;
    gap: 0.5rem;
    align-items: center;
  }
  .ico {
    display: grid;
    place-items: center;
    width: 2rem;
    height: 2rem;
    border-radius: 8px;
    background: var(--accent-soft);
    color: var(--accent);
  }
  .path {
    margin: 0.6rem 0 0.9rem;
    padding: 0.5rem 0.7rem;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    word-break: break-all;
  }
  .empty {
    margin: 0.6rem 0 0.9rem;
  }
</style>
