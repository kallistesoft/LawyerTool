<script lang="ts">
  import { onMount } from 'svelte'
  import { onWatchUpdated, vaultLock, watchStatus, type AppInfo } from './api'
  import AssessmentView from './AssessmentView.svelte'
  import ClientForm from './ClientForm.svelte'
  import ClientList from './ClientList.svelte'
  import Icon from './Icon.svelte'
  import Settings from './Settings.svelte'
  import WatchView from './WatchView.svelte'

  let { info, dev }: { info: AppInfo | null; dev: boolean } = $props()

  type View = { kind: 'list' } | { kind: 'edit'; id: number | null } | { kind: 'assess'; id: number } | { kind: 'watch' } | { kind: 'settings' }
  let view = $state<View>({ kind: 'list' })
  const list = () => (view = { kind: 'list' })
  const tab = $derived(view.kind === 'watch' || view.kind === 'settings' ? view.kind : 'clients')

  // Badge on "Veille": changes waiting for approval, or rules not checked lately.
  let watchAlert = $state<'pending' | 'stale' | null>(null)
  const refreshWatch = () =>
    watchStatus()
      .then((s) => (watchAlert = s.pending ? 'pending' : s.stale ? 'stale' : null))
      .catch(() => {})
  onMount(() => {
    refreshWatch()
    const un = onWatchUpdated(refreshWatch)
    return () => {
      un.then((f) => f())
    }
  })
  $effect(() => {
    if (view.kind) refreshWatch()
  })

  const nav = [
    { key: 'clients', label: 'Clients', icon: 'clients', go: list },
    { key: 'watch', label: 'Veille des règles', icon: 'rules', go: () => (view = { kind: 'watch' }) },
    { key: 'settings', label: 'Paramètres', icon: 'settings', go: () => (view = { kind: 'settings' }) },
  ] as const
</script>

<div class="shell">
  <aside class="sidebar">
    <div class="brand">
      <span class="logo"><Icon name="scale" size={18} /></span>
      <span>LawyerTool</span>
    </div>
    <nav>
      {#each nav as n (n.key)}
        <button class="nav-item" class:active={tab === n.key} onclick={n.go}>
          <Icon name={n.icon} size={17} />
          <span>{n.label}</span>
          {#if n.key === 'watch' && watchAlert}
            <span class="dot {watchAlert}" title={watchAlert === 'pending' ? 'Modifications à examiner' : 'Règles non vérifiées depuis 7 jours'}></span>
          {/if}
        </button>
      {/each}
    </nav>
    <div class="foot">
      {#if dev}
        <span class="badge warn" title="Coffre de test automatique, sans phrase de passe ni verrouillage. Aucune donnée réelle.">MODE DEV</span>
      {:else}
        <button class="nav-item" onclick={() => vaultLock()}><Icon name="lock" size={17} /><span>Verrouiller</span></button>
      {/if}
      {#if info}<div class="version">Version {info.version}</div>{/if}
    </div>
  </aside>

  <main class="content">
    {#if view.kind === 'watch'}
      <WatchView />
    {:else if view.kind === 'settings'}
      <Settings />
    {:else if view.kind === 'list'}
      <ClientList
        onOpen={(id) => (view = { kind: 'assess', id })}
        onNew={() => (view = { kind: 'edit', id: null })}
        onSettings={() => (view = { kind: 'settings' })}
      />
    {:else if view.kind === 'edit'}
      {#key view.id}
        <ClientForm id={view.id} onSaved={() => {}} onAssess={(id) => (view = { kind: 'assess', id })} onBack={list} />
      {/key}
    {:else}
      {#key view.id}
        <AssessmentView id={view.id} onEdit={(id) => (view = { kind: 'edit', id })} onBack={list} />
      {/key}
    {/if}
  </main>
</div>

<style>
  .shell {
    display: grid;
    grid-template-columns: 15rem 1fr;
    height: 100vh;
  }
  .sidebar {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    padding: 1.1rem 0.8rem;
    background: var(--accent);
    color: #dfe8f3;
  }
  @media (prefers-color-scheme: dark) {
    .sidebar {
      background: #0c1118;
      border-right: 1px solid var(--border);
    }
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.2rem 0.5rem 0.8rem;
    font-weight: 700;
    font-size: 1.05rem;
    color: #fff;
  }
  .logo {
    display: grid;
    place-items: center;
    width: 2rem;
    height: 2rem;
    border-radius: 8px;
    background: rgb(255 255 255 / 14%);
  }
  nav {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }
  .nav-item {
    width: 100%;
    justify-content: flex-start;
    gap: 0.65rem;
    padding: 0.55rem 0.7rem;
    background: transparent;
    border: none;
    color: inherit;
    font-weight: 550;
  }
  .nav-item:hover:not(:disabled) {
    background: rgb(255 255 255 / 9%);
    border: none;
  }
  .nav-item.active {
    background: rgb(255 255 255 / 16%);
    color: #fff;
  }
  .dot {
    margin-left: auto;
    width: 0.55rem;
    height: 0.55rem;
    border-radius: 50%;
    background: #f0b45c;
  }
  .dot.pending {
    background: #ff8a8a;
    box-shadow: 0 0 0 3px rgb(255 138 138 / 25%);
  }
  .foot {
    margin-top: auto;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    align-items: flex-start;
  }
  .version {
    padding: 0 0.7rem;
    font-size: 0.75rem;
    opacity: 0.6;
  }
  .content {
    overflow-y: auto;
    padding: 1.6rem 2rem 3rem;
  }
  .content > :global(*) {
    max-width: 78rem;
    margin-left: auto;
    margin-right: auto;
  }
  @media (max-width: 760px) {
    .shell {
      grid-template-columns: 4.2rem 1fr;
    }
    .brand span:last-child,
    .nav-item span,
    .version {
      display: none;
    }
  }
</style>
