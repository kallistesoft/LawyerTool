<script lang="ts">
  import { errorMessage, vaultChoose, vaultCreate } from './api'
  import PassphraseFields from './PassphraseFields.svelte'

  let { onCreated, onOpened }: { onCreated: (phrase: string) => void; onOpened: () => void } = $props()
  let passphrase = $state('')
  let valid = $state(false)
  let busy = $state(false)
  let error: string | null = $state(null)

  async function create(e: SubmitEvent) {
    e.preventDefault()
    busy = true
    error = null
    try {
      const created = await vaultCreate(passphrase)
      if (!created) return // "Enregistrer sous" annulé
      passphrase = ''
      onCreated(created.recovery_phrase)
    } catch (err) {
      error = errorMessage(err)
    } finally {
      busy = false
    }
  }

  async function open() {
    error = null
    try {
      if (await vaultChoose()) onOpened()
    } catch (err) {
      error = errorMessage(err)
    }
  }
</script>

<form novalidate onsubmit={create}>
  <h2>Créer le coffre chiffré</h2>
  <p class="hint">
    Les dossiers clients sont chiffrés dans un seul fichier, sur cet ordinateur ; ils ne sont jamais envoyés au site.
    Vous choisirez où enregistrer ce fichier (un dossier sauvegardé, de préférence). La phrase de passe déverrouille
    le coffre ; elle n'est jamais enregistrée.
  </p>
  <PassphraseFields bind:value={passphrase} bind:valid />
  {#if error}<p class="callout err">{error}</p>{/if}
  <div class="actions auth-actions">
    <button type="submit" class="primary" disabled={!valid || busy}>{busy ? 'Création…' : 'Créer le coffre…'}</button>
    <button type="button" class="ghost" onclick={open}>J’ai déjà un coffre : l’ouvrir…</button>
  </div>
</form>
