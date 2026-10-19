<script lang="ts">
  import { errorMessage, vaultCreate } from './api'
  import PassphraseFields from './PassphraseFields.svelte'

  let { onCreated }: { onCreated: (phrase: string) => void } = $props()
  let passphrase = $state('')
  let valid = $state(false)
  let busy = $state(false)
  let error: string | null = $state(null)

  async function create(e: SubmitEvent) {
    e.preventDefault()
    busy = true
    error = null
    try {
      const { recovery_phrase } = await vaultCreate(passphrase)
      passphrase = ''
      onCreated(recovery_phrase)
    } catch (err) {
      error = errorMessage(err)
    } finally {
      busy = false
    }
  }
</script>

<form novalidate onsubmit={create}>
  <h2>Créer le coffre chiffré</h2>
  <p class="hint">
    Les dossiers clients sont chiffrés sur cet ordinateur. La phrase de passe déverrouille le coffre ; elle n'est
    jamais enregistrée.
  </p>
  <PassphraseFields bind:value={passphrase} bind:valid />
  {#if error}<p class="callout err">{error}</p>{/if}
  <button type="submit" class="primary auth-submit" disabled={!valid || busy}>{busy ? 'Création…' : 'Créer le coffre'}</button>
</form>
