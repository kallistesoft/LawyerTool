<script lang="ts">
  import { onMount } from 'svelte'
  import { appInfo, onLocked, reportActivity, vaultLock, vaultStatus, type AppInfo } from './lib/api'
  import Recover from './lib/Recover.svelte'
  import RecoveryShow from './lib/RecoveryShow.svelte'
  import Setup from './lib/Setup.svelte'
  import Unlock from './lib/Unlock.svelte'
  import UpdateBanner from './lib/UpdateBanner.svelte'
  import Workspace from './lib/Workspace.svelte'

  type Screen = 'loading' | 'setup' | 'recovery-show' | 'unlock' | 'recover' | 'home'
  let screen: Screen = $state('loading')
  let phrase = $state('')
  let info: AppInfo | null = $state(null)
  let idleMinutes = $state(5)
  let dev = $state(false)

  async function refresh() {
    const s = await vaultStatus()
    idleMinutes = s.idle_minutes
    dev = s.dev
    screen = s.unlocked ? 'home' : s.exists ? 'unlock' : 'setup'
  }

  onMount(() => {
    appInfo().then((i) => (info = i))
    refresh()
    const unlisten = onLocked(() => {
      phrase = ''
      screen = 'unlock'
    })
    return () => {
      unlisten.then((f) => f())
    }
  })
</script>

<svelte:window onmousemove={reportActivity} onkeydown={reportActivity} onclick={reportActivity} />

{#if dev}
  <div class="dev-banner">MODE DEV : coffre de test automatique, sans phrase de passe ni verrouillage. Aucune donnée réelle.</div>
{/if}

<main>
  <header>
    <h1>LawyerTool</h1>
    {#if screen === 'home' && !dev}<button class="link" onclick={() => vaultLock()}>Verrouiller</button>{/if}
  </header>

  {#if screen === 'setup'}
    <Setup onCreated={(p) => ((phrase = p), (screen = 'recovery-show'))} />
  {:else if screen === 'recovery-show'}
    <RecoveryShow {phrase} onDone={() => ((phrase = ''), (screen = 'home'))} />
  {:else if screen === 'unlock'}
    <Unlock onUnlocked={() => (screen = 'home')} onRecover={() => (screen = 'recover')} />
  {:else if screen === 'recover'}
    <Recover onRecovered={() => (screen = 'home')} onCancel={() => (screen = 'unlock')} />
  {:else if screen === 'home'}
    <UpdateBanner />
    <Workspace />
  {/if}

  {#if info}<footer>{info.name} {info.version}</footer>{/if}
</main>

<style>
  .dev-banner {
    background: #c77700;
    color: #fff;
    padding: 0.4rem 1rem;
    font: 600 0.85rem system-ui, sans-serif;
    text-align: center;
  }
  main {
    max-width: 60rem;
    margin: 2rem auto;
    padding: 0 1.5rem;
    font-family: system-ui, sans-serif;
    line-height: 1.45;
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  footer {
    margin-top: 3rem;
    font-size: 0.8rem;
    opacity: 0.6;
  }
  :global(label) {
    display: block;
    margin: 0.8rem 0;
  }
  :global(label.check) {
    display: flex;
    gap: 0.5rem;
    align-items: center;
  }
  :global(input:not([type='checkbox'])),
  :global(textarea) {
    display: block;
    width: 100%;
    box-sizing: border-box;
    padding: 0.5rem;
    margin-top: 0.25rem;
    font: inherit;
  }
  :global(button) {
    margin: 0.8rem 0.5rem 0 0;
    padding: 0.5rem 1rem;
    font: inherit;
  }
  :global(button.link) {
    background: none;
    border: none;
    text-decoration: underline;
    cursor: pointer;
    padding: 0;
  }
  :global(.error) {
    color: #b00020;
  }
  :global(.hint) {
    font-size: 0.85rem;
    opacity: 0.8;
  }
  :global(.warn) {
    border-left: 4px solid #c77700;
    padding-left: 0.8rem;
  }
  :global(fieldset) {
    border: 1px solid color-mix(in srgb, currentColor 20%, transparent);
    border-radius: 6px;
    margin: 1rem 0;
    padding: 0.6rem 1rem;
  }
  :global(legend) {
    font-weight: 600;
    padding: 0 0.3rem;
  }
  :global(.row) {
    display: flex;
    gap: 1rem;
    flex-wrap: wrap;
    align-items: end;
  }
  :global(.row > label) {
    flex: 1 1 10rem;
  }
  :global(.between) {
    justify-content: space-between;
    align-items: center;
  }
  :global(label.inline) {
    display: flex;
    justify-content: space-between;
    gap: 1rem;
    align-items: center;
  }
  :global(label.inline select) {
    width: auto;
  }
  :global(select) {
    display: block;
    width: 100%;
    padding: 0.45rem;
    margin-top: 0.25rem;
    font: inherit;
  }
  :global(.card) {
    border-left: 3px solid color-mix(in srgb, currentColor 25%, transparent);
    padding-left: 0.8rem;
    margin: 0.8rem 0;
  }
  :global(table) {
    width: 100%;
    border-collapse: collapse;
    margin: 0.5rem 0;
  }
  :global(td),
  :global(th) {
    text-align: left;
    padding: 0.35rem 0.5rem;
    border-bottom: 1px solid color-mix(in srgb, currentColor 12%, transparent);
    vertical-align: top;
  }
  :global(tr.clickable) {
    cursor: pointer;
  }
  :global(tr.clickable:hover) {
    background: color-mix(in srgb, currentColor 6%, transparent);
  }
  :global(details) {
    margin: 0.6rem 0;
  }
  :global(summary) {
    cursor: pointer;
  }
  :global(button.danger) {
    color: #fff;
    background: #b00020;
    border: none;
    border-radius: 4px;
  }
  :global(ol.words) {
    columns: 3;
    font-family: ui-monospace, monospace;
    font-size: 1.05rem;
  }
</style>
