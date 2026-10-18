<script lang="ts">
  import { onMount } from 'svelte'
  import { clientList, errorMessage } from './api'
  import type { ClientSummary } from './model'

  let { onOpen, onNew }: { onOpen: (id: number) => void; onNew: () => void } = $props()
  let clients: ClientSummary[] = $state([])
  let error: string | null = $state(null)
  let filter = $state('')
  let shown = $derived(
    clients.filter((c) => (c.full_name + ' ' + (c.file_reference ?? '')).toLowerCase().includes(filter.toLowerCase())),
  )

  onMount(() => {
    clientList().then((c) => (clients = c)).catch((e) => (error = errorMessage(e)))
  })
</script>

<section>
  <div class="row between">
    <h2>Clients</h2>
    <button onclick={onNew}>Nouveau client</button>
  </div>
  <input placeholder="Rechercher (nom, dossier)" bind:value={filter} />
  {#if error}<p class="error">{error}</p>{/if}
  {#if clients.length === 0}
    <p class="hint">Aucun client pour l’instant.</p>
  {:else}
    <table>
      <thead><tr><th>Nom</th><th>Dossier</th><th>Modifié</th></tr></thead>
      <tbody>
        {#each shown as c (c.id)}
          <tr class="clickable" onclick={() => onOpen(c.id)}>
            <td>{c.full_name}</td><td>{c.file_reference ?? ''}</td><td>{c.updated_at.slice(0, 10)}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
</section>
