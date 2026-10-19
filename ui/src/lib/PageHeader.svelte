<script lang="ts">
  import type { Snippet } from 'svelte'
  let { title, crumbs = [], subtitle, actions }: {
    title: string
    crumbs?: { label: string; go: () => void }[]
    subtitle?: string
    actions?: Snippet
  } = $props()
</script>

<header class="page-header">
  <div>
    {#if crumbs.length}
      <div class="crumbs">
        {#each crumbs as c (c.label)}<button class="link" onclick={c.go}>{c.label}</button><span>›</span>{/each}
      </div>
    {/if}
    <h1>{title}</h1>
    {#if subtitle}<p class="hint">{subtitle}</p>{/if}
  </div>
  {#if actions}<div class="actions">{@render actions()}</div>{/if}
</header>

<style>
  .page-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    gap: 1rem;
    flex-wrap: wrap;
    margin-bottom: 1.3rem;
  }
  h1 {
    margin: 0;
  }
  .crumbs {
    display: flex;
    gap: 0.4rem;
    font-size: 0.85rem;
    color: var(--muted);
    margin-bottom: 0.2rem;
  }
</style>
