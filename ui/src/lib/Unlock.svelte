<script lang="ts">
  import { errorMessage, vaultUnlock } from './api'

  let { onUnlocked, onRecover }: { onUnlocked: () => void; onRecover: () => void } = $props()
  let passphrase = $state('')
  let busy = $state(false)
  let error: string | null = $state(null)
  let code: string | null = $state(null)

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
      code = (err as { code?: string }).code ?? null
    } finally {
      busy = false
    }
  }
</script>

<form novalidate onsubmit={unlock}>
  <h2>Coffre verrouillé</h2>
  <label>
    Phrase de passe
    <!-- svelte-ignore a11y_autofocus -->
    <input type="password" autocomplete="current-password" autofocus bind:value={passphrase} />
  </label>
  {#if error}<p class="callout err">{error}</p>{/if}
  <div class="actions auth-actions">
  <button type="submit" class="primary" disabled={!passphrase || busy}>{busy ? 'Vérification…' : 'Déverrouiller'}</button>
  <button type="button" class={code === 'keychain_missing' ? '' : 'ghost'} onclick={onRecover}>
    Utiliser la phrase de récupération
  </button>
  </div>
</form>
