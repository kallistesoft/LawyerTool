<script lang="ts">
  import AssessmentView from './AssessmentView.svelte'
  import ClientForm from './ClientForm.svelte'
  import ClientList from './ClientList.svelte'
  import WatchView from './WatchView.svelte'

  type View = { kind: 'list' } | { kind: 'edit'; id: number | null } | { kind: 'assess'; id: number } | { kind: 'watch' }
  let view: View = $state({ kind: 'list' })
  const list = () => (view = { kind: 'list' })
</script>

<nav class="tabs">
  <button class={view.kind === 'watch' ? 'link' : 'link active'} onclick={list}>Clients</button>
  <button class={view.kind === 'watch' ? 'link active' : 'link'} onclick={() => (view = { kind: 'watch' })}>Veille des règles</button>
</nav>

{#if view.kind === 'watch'}
  <WatchView />
{:else if view.kind === 'list'}
  <ClientList onOpen={(id) => (view = { kind: 'assess', id })} onNew={() => (view = { kind: 'edit', id: null })} />
{:else if view.kind === 'edit'}
  {#key view.id}
    <ClientForm id={view.id} onSaved={() => {}} onAssess={(id) => (view = { kind: 'assess', id })} onBack={list} />
  {/key}
{:else}
  {#key view.id}
    <AssessmentView id={view.id} onEdit={(id) => (view = { kind: 'edit', id })} onBack={list} />
  {/key}
{/if}

<style>
  .tabs {
    display: flex;
    gap: 1.5rem;
    border-bottom: 1px solid color-mix(in srgb, currentColor 15%, transparent);
    margin-bottom: 1rem;
  }
  .tabs :global(button.active) {
    font-weight: 700;
    text-decoration: none;
  }
</style>
