<script lang="ts">
  import { errorMessage, vaultRecover } from './api'
  import PassphraseFields from './PassphraseFields.svelte'

  let { onRecovered, onCancel }: { onRecovered: () => void; onCancel: () => void } = $props()
  let phrase = $state('')
  let passphrase = $state('')
  let valid = $state(false)
  let busy = $state(false)
  let error: string | null = $state(null)
  let count = $derived(phrase.trim() ? phrase.trim().split(/\s+/).length : 0)

  async function recover(e: SubmitEvent) {
    e.preventDefault()
    busy = true
    error = null
    try {
      await vaultRecover(phrase, passphrase)
      phrase = ''
      passphrase = ''
      onRecovered()
    } catch (err) {
      error = errorMessage(err)
    } finally {
      busy = false
    }
  }
</script>

<form novalidate onsubmit={recover}>
  <h2>Récupération</h2>
  <p>Saisissez les 24 mots, puis choisissez une nouvelle phrase de passe pour cet ordinateur.</p>
  <label>
    Phrase de récupération ({count}/24)
    <textarea rows="4" autocomplete="off" spellcheck="false" bind:value={phrase}></textarea>
  </label>
  <PassphraseFields bind:value={passphrase} bind:valid label="Nouvelle phrase de passe" />
  {#if error}<p class="error">{error}</p>{/if}
  <button type="submit" disabled={count !== 24 || !valid || busy}>{busy ? 'Récupération…' : 'Récupérer'}</button>
  <button type="button" class="link" onclick={onCancel}>Annuler</button>
</form>
