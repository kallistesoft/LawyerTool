<script lang="ts">
  import { onMount } from 'svelte'
  import { appInfo, onLocked, reportActivity, vaultStatus, type AppInfo } from './lib/api'
  import Icon from './lib/Icon.svelte'
  import Recover from './lib/Recover.svelte'
  import RecoveryShow from './lib/RecoveryShow.svelte'
  import Setup from './lib/Setup.svelte'
  import Unlock from './lib/Unlock.svelte'
  import Workspace from './lib/Workspace.svelte'

  type Screen = 'loading' | 'setup' | 'recovery-show' | 'unlock' | 'recover' | 'home'
  let screen: Screen = $state('loading')
  let phrase = $state('')
  let info: AppInfo | null = $state(null)
  let dev = $state(false)

  async function refresh() {
    const s = await vaultStatus()
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

{#if screen === 'home'}
  <Workspace {info} {dev} />
{:else if screen !== 'loading'}
  <div class="auth">
    <div class="auth-card card">
      <div class="brand">
        <span class="logo"><Icon name="scale" size={22} /></span>
        <div>
          <strong>LawyerTool</strong>
          <div class="hint">Admissibilité Entrée express · données chiffrées sur cet ordinateur</div>
        </div>
      </div>
      {#if screen === 'setup'}
        <Setup onCreated={(p) => ((phrase = p), (screen = 'recovery-show'))} />
      {:else if screen === 'recovery-show'}
        <RecoveryShow {phrase} onDone={() => ((phrase = ''), (screen = 'home'))} />
      {:else if screen === 'unlock'}
        <Unlock onUnlocked={() => (screen = 'home')} onRecover={() => (screen = 'recover')} />
      {:else if screen === 'recover'}
        <Recover onRecovered={() => (screen = 'home')} onCancel={() => (screen = 'unlock')} />
      {/if}
    </div>
    {#if info}<p class="hint version">{info.name} {info.version}</p>{/if}
  </div>
{/if}

<style>
  .auth {
    min-height: 100vh;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 2rem;
    background: radial-gradient(circle at 20% 10%, var(--accent-soft), transparent 60%), var(--bg);
  }
  .auth-card {
    width: min(34rem, 100%);
    padding: 1.8rem 2rem;
  }
  .brand {
    display: flex;
    gap: 0.8rem;
    align-items: center;
    margin-bottom: 1.4rem;
    padding-bottom: 1rem;
    border-bottom: 1px solid var(--border);
  }
  .brand strong {
    font-size: 1.15rem;
  }
  .logo {
    display: grid;
    place-items: center;
    width: 2.4rem;
    height: 2.4rem;
    border-radius: 10px;
    background: var(--accent);
    color: #fff;
  }
  .version {
    margin-top: 1rem;
  }
</style>
