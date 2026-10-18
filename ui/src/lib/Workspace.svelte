<script lang="ts">
  import AssessmentView from './AssessmentView.svelte'
  import ClientForm from './ClientForm.svelte'
  import ClientList from './ClientList.svelte'

  type View = { kind: 'list' } | { kind: 'edit'; id: number | null } | { kind: 'assess'; id: number }
  let view: View = $state({ kind: 'list' })
  const list = () => (view = { kind: 'list' })
</script>

{#if view.kind === 'list'}
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
