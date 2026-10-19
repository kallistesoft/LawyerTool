<script lang="ts">
  import { onMount } from 'svelte'
  import { errorMessage, updateCheck, updateInstall, type UpdateInfo } from './api'

  let info: UpdateInfo | null = $state(null)
  let busy = $state(false)
  let error: string | null = $state(null)
  let dismissed = $state(false)

  onMount(() => {
    updateCheck()
      .then((i) => (info = i))
      .catch(() => {}) // offline or not configured: no banner
  })

  async function install() {
    busy = true
    error = null
    try {
      await updateInstall() // the app restarts on success
    } catch (e) {
      error = errorMessage(e)
      busy = false
    }
  }
</script>

{#if info?.available && (info.mandatory || !dismissed)}
  <div class={info.mandatory ? 'update mandatory' : 'update callout info'} role="alert">
    <p>
      {#if info.mandatory}
        <strong>Mise à jour obligatoire</strong> : la version {info.current} n'est plus prise en charge.
      {:else}
        <strong>Mise à jour disponible</strong> : version {info.version} (vous avez {info.current}).
      {/if}
    </p>
    {#if info.notes}<p class="notes">{info.notes}</p>{/if}
    {#if error}<p class="error">{error}</p>{/if}
    <div class="actions">
    <button class="primary" disabled={busy} onclick={install}>{busy ? 'Installation…' : 'Installer et redémarrer'}</button>
    {#if !info.mandatory}<button class="ghost" onclick={() => (dismissed = true)}>Plus tard</button>{/if}
    </div>
  </div>
{/if}

<style>
  .update {
    margin: 0 0 1rem;
  }
  .mandatory {
    position: fixed;
    inset: 0;
    margin: 0;
    border-radius: 0;
    background: var(--bg);
    z-index: 10;
    padding: 20vh 15vw;
  }
  .notes {
    white-space: pre-wrap;
    font-size: 0.9rem;
  }
</style>
