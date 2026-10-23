<script lang="ts">
  import { onMount } from 'svelte'
  import {
    errorMessage, folderAccess, generateAudits, inboxList, settingsGet,
    type AuditResult, type FolderStatus, type Settings,
  } from './api'
  import Icon from './Icon.svelte'

  /** Home: one action, « Générer les audits », with the folders' status and the results. */
  let { onOpen, onSettings, onClients }: {
    onOpen: (id: number) => void
    onSettings: () => void
    onClients: () => void
  } = $props()

  let settings: Settings | null = $state(null)
  let forms: number | null = $state(null)
  let results: AuditResult[] = $state([])
  let error: string | null = $state(null)
  let busy = $state(false)
  let done = $state(false)

  async function refresh() {
    settings = await settingsGet().catch(() => null)
    forms = null
    if (settings?.input_dir && !settings.input_dir.needs_permission) {
      forms = await inboxList().then((f) => f.filter((x) => x.status !== 'too_large').length).catch(() => null)
    }
  }
  onMount(refresh)

  const ready = (s: FolderStatus | null | undefined) => !!s && s.exists && !s.needs_permission
  // Read through a function: TypeScript narrows `settings` to its initial null otherwise.
  const current = (): Settings | null => settings
  const canGenerate = $derived(!!current()?.input_dir && !!current()?.output_dir)
  const needsAccess = $derived(!!(current()?.input_dir?.needs_permission || current()?.output_dir?.needs_permission))
  const written = $derived(results.filter((r) => r.ok))

  async function generate() {
    busy = true
    error = null
    done = false
    results = []
    try {
      results = await generateAudits()
      done = true
      await refresh()
    } catch (e) {
      error = errorMessage(e)
    } finally {
      busy = false
    }
  }

  async function allow() {
    await folderAccess().catch(() => {})
    await refresh()
  }
</script>

<section class="hero">
  <div class="hero-text">
    <span class="eyebrow">Audit éclair d’éligibilité</span>
    <h1>Générer les audits</h1>
    <p>
      Les réponses du formulaire (dossier d’entrée) deviennent un audit Word par client, prêt à finaliser, dans le
      dossier de sortie. Les passages surlignés en jaune sont à compléter.
    </p>
  </div>

  <div class="folders">
    <button class="folder" class:ok={ready(settings?.input_dir)} onclick={onSettings}>
      <span class="ico"><Icon name="inbox" size={18} /></span>
      <span class="what">
        <span class="label">Formulaires</span>
        <span class="value">{settings?.input_dir?.path ?? 'Choisir le dossier…'}</span>
      </span>
      {#if forms !== null}<span class="count">{forms} fichier{forms > 1 ? 's' : ''}</span>{/if}
    </button>
    <span class="arrow" aria-hidden="true">→</span>
    <button class="folder" class:ok={ready(settings?.output_dir)} onclick={onSettings}>
      <span class="ico"><Icon name="folder" size={18} /></span>
      <span class="what">
        <span class="label">Audits Word</span>
        <span class="value">{settings?.output_dir?.path ?? 'Choisir le dossier…'}</span>
      </span>
    </button>
  </div>

  {#if needsAccess}
    <button class="cta" onclick={allow}><Icon name="lock" size={20} />Autoriser l’accès aux dossiers</button>
  {:else}
    <button class="cta" onclick={generate} disabled={busy || !canGenerate}>
      {#if busy}
        <span class="spinner" aria-hidden="true"></span>Génération en cours…
      {:else}
        <Icon name="download" size={20} />Générer les audits
      {/if}
    </button>
  {/if}
  {#if !canGenerate && settings}
    <p class="hint center">Choisissez d’abord les deux dossiers dans <button class="link" onclick={onSettings}>Paramètres</button>.</p>
  {/if}
  {#if error}<p class="callout err">{error}</p>{/if}
</section>

{#if done}
  <section class="results card">
    <div class="results-head">
      <span class="badge-big"><Icon name="check" size={18} /></span>
      <div>
        <strong>{written.length} audit{written.length > 1 ? 's' : ''} enregistré{written.length > 1 ? 's' : ''}</strong>
        <div class="hint">dans « {settings?.output_dir?.path} » · ouvrez-les dans Word pour finaliser</div>
      </div>
    </div>
    {#if results.length}
      <ul>
        {#each results as r (r.client_id)}
          <li class:fail={!r.ok}>
            <Icon name={r.ok ? 'download' : 'alert'} size={15} />
            <span class="file">{r.file ?? `Client ${r.client_id}`}</span>
            {#if !r.ok}<span class="hint">{r.message}</span>{/if}
            <button class="link" onclick={() => onOpen(r.client_id)}>CNP et notes</button>
          </li>
        {/each}
      </ul>
    {:else}
      <p class="hint">Aucune réponse trouvée dans le dossier des formulaires.</p>
    {/if}
  </section>
{/if}

<p class="more"><button class="link" onclick={onClients}>Afficher les dossiers clients</button></p>

<style>
  .hero {
    max-width: 52rem;
    margin: 1.5rem auto 1rem;
    padding: 2.4rem 2.4rem 2rem;
    border-radius: 18px;
    background:
      radial-gradient(120% 120% at 100% 0%, color-mix(in srgb, var(--accent-2) 18%, transparent), transparent 60%),
      var(--surface);
    border: 1px solid var(--border);
    box-shadow: 0 10px 30px rgba(15, 23, 42, 0.08);
    text-align: center;
  }
  .eyebrow {
    font-size: 0.75rem;
    font-weight: 700;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--accent-2);
  }
  h1 {
    margin: 0.4rem 0 0.6rem;
    font-size: 2.1rem;
    letter-spacing: -0.02em;
  }
  .hero-text p {
    max-width: 36rem;
    margin: 0 auto;
    color: var(--muted);
  }
  .folders {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.8rem;
    margin: 1.8rem 0;
    flex-wrap: wrap;
  }
  .folder {
    display: flex;
    align-items: center;
    gap: 0.7rem;
    min-width: 15rem;
    padding: 0.8rem 1rem;
    border-radius: 12px;
    border: 1px dashed var(--border);
    background: var(--surface-2);
    text-align: left;
  }
  .folder.ok {
    border-style: solid;
  }
  .folder .ico {
    display: grid;
    place-items: center;
    width: 2.2rem;
    height: 2.2rem;
    border-radius: 10px;
    background: var(--accent-soft);
    color: var(--accent-2);
  }
  .folder.ok .ico {
    background: color-mix(in srgb, var(--ok) 15%, transparent);
    color: var(--ok);
  }
  .what {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }
  .label {
    font-size: 0.72rem;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--muted);
  }
  .value {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .count {
    font-size: 0.8rem;
    padding: 0.15rem 0.5rem;
    border-radius: 999px;
    background: var(--accent-soft);
    color: var(--accent-2);
    font-weight: 600;
  }
  .arrow {
    color: var(--muted);
    font-size: 1.3rem;
  }
  .cta {
    display: inline-flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.95rem 2.2rem;
    font-size: 1.08rem;
    font-weight: 700;
    color: #fff;
    border: 0;
    border-radius: 999px;
    background: linear-gradient(135deg, var(--accent), var(--accent-2));
    box-shadow: 0 8px 20px color-mix(in srgb, var(--accent) 35%, transparent);
    transition: transform 0.12s ease, box-shadow 0.12s ease;
  }
  .cta:hover:not(:disabled) {
    transform: translateY(-1px);
    box-shadow: 0 12px 26px color-mix(in srgb, var(--accent) 40%, transparent);
    background: linear-gradient(135deg, var(--accent), var(--accent-2));
  }
  .cta:disabled {
    opacity: 0.55;
    box-shadow: none;
  }
  .spinner {
    width: 1rem;
    height: 1rem;
    border: 2px solid rgba(255, 255, 255, 0.4);
    border-top-color: #fff;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .center {
    text-align: center;
    margin-top: 0.8rem;
  }
  .results {
    max-width: 52rem;
    margin: 0 auto;
    padding: 1.2rem 1.4rem;
  }
  .results-head {
    display: flex;
    gap: 0.8rem;
    align-items: center;
    margin-bottom: 0.6rem;
  }
  .badge-big {
    display: grid;
    place-items: center;
    width: 2.4rem;
    height: 2.4rem;
    border-radius: 50%;
    background: color-mix(in srgb, var(--ok) 15%, transparent);
    color: var(--ok);
  }
  .results ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .results li {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.5rem 0.2rem;
    border-top: 1px solid var(--border);
  }
  .results li .file {
    flex: 1;
    font-weight: 600;
  }
  .results li.fail {
    color: var(--err);
  }
  .more {
    text-align: center;
    margin-top: 1.4rem;
  }
</style>
