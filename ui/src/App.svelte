<script lang="ts">
  import { appInfo, type AppInfo } from './lib/api'

  let info: AppInfo | null = $state(null)
  let error: string | null = $state(null)

  appInfo().then((i) => (info = i)).catch((e) => (error = String(e)))
</script>

<main>
  <h1>LawyerTool</h1>
  {#if info}
    <p>{info.name} {info.version}</p>
  {:else if error}
    <p class="error">{error}</p>
  {/if}
</main>

<style>
  main {
    max-width: 40rem;
    margin: 4rem auto;
    font-family: system-ui, sans-serif;
  }
  .error {
    color: #b00020;
  }
</style>
