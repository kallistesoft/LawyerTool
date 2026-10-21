<script lang="ts">
  import { errorMessage, vaultChoose, vaultUnlock } from './api'

  let { file, onUnlocked, onRecover, onOpened }: {
    file: string | null
    onUnlocked: () => void
    onRecover: () => void
    onOpened: () => void
  } = $props()
  let passphrase = $state('')
  let busy = $state(false)
  let error: string | null = $state(null)

  async function unlock(e: SubmitEvent) {
    e.preventDefault()
    busy = true
    error = null
    try {
      await vaultUnlock(passphrase)
      passphrase = ''
      onUnlocked()
    } catch (err) {
      error = errorMessage(err)
    } finally {
      busy = false
    }
  }

  async function other() {
    error = null
    try {
      if (await vaultChoose()) onOpened()
    } catch (err) {
      error = errorMessage(err)
    }
  }
</script>

<form novalidate onsubmit={unlock}>
  <h2>Coffre verrouillé</h2>
  {#if file}<p class="hint">Fichier : <strong>{file}</strong> <button type="button" class="link" onclick={other}>Ouvrir un autre coffre…</button></p>{/if}
  <label>
    Phrase de passe
    <!-- svelte-ignore a11y_autofocus -->
    <input type="password" autocomplete="current-password" autofocus bind:value={passphrase} />
  </label>
  {#if error}<p class="callout err">{error}</p>{/if}
  <div class="actions auth-actions">
  <button type="submit" class="primary" disabled={!passphrase || busy}>{busy ? 'Vérification…' : 'Déverrouiller'}</button>
  <button type="button" class="ghost" onclick={onRecover}>
    Utiliser la phrase de récupération
  </button>
  </div>
</form>
