<script lang="ts">
  import Icon from './Icon.svelte'

  /** To-complete notes left by a form import, until the lawyer clears them. */
  let { notes, onClear }: { notes: string[]; onClear?: () => void } = $props()
  let open = $state(true)
</script>

{#if notes.length}
  <div class="callout warn notes">
    <div class="head">
      <button type="button" class="ghost toggle" onclick={() => (open = !open)} aria-expanded={open}>
        <Icon name="alert" size={16} />
        <strong>À compléter : profil importé du formulaire ({notes.length})</strong>
        <span class="chev">{open ? '▾' : '▸'}</span>
      </button>
      {#if onClear}
        <button type="button" class="small" onclick={onClear}><Icon name="check" size={14} />Tout est complété</button>
      {/if}
    </div>
    {#if open}
      <ul>
        {#each notes as n, i (i)}<li>{n}</li>{/each}
      </ul>
    {/if}
  </div>
{/if}

<style>
  .notes {
    margin-bottom: 1rem;
  }
  .head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 1rem;
    flex-wrap: wrap;
  }
  .toggle {
    gap: 0.5rem;
    padding: 0.2rem 0.3rem;
    text-align: left;
  }
  .chev {
    color: var(--muted);
  }
  ul {
    margin: 0.5rem 0 0.2rem;
    padding-left: 1.3rem;
    font-size: 0.9rem;
  }
  li + li {
    margin-top: 0.25rem;
  }
</style>
