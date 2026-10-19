<script lang="ts">
  let { phrase, onDone }: { phrase: string; onDone: () => void } = $props()
  const words = $derived(phrase.split(' '))
  // Ask for two words to make sure the phrase was written down.
  const picks = $derived.by(() => {
    const a = 1 + Math.floor(Math.random() * 12)
    const b = 13 + Math.floor(Math.random() * 12)
    return [a, b]
  })
  let answers = $state(['', ''])
  let written = $state(false)
  let ok = $derived(
    written && picks.every((n, i) => answers[i].trim().toLowerCase() === words[n - 1]),
  )
</script>

<section>
  <h2>Phrase de récupération</h2>
  <p class="callout warn">
    Écrivez ces 24 mots sur papier et conservez-les dans un endroit sûr (coffre du cabinet). Ils sont affichés une
    seule fois. Sans eux, un ordinateur perdu ou réinstallé signifie la perte des dossiers.
  </p>
  <ol class="words">
    {#each words as w, i (i)}<li>{w}</li>{/each}
  </ol>
  <label class="check"><input type="checkbox" bind:checked={written} /> J'ai écrit les 24 mots.</label>
  {#each picks as n, i (n)}
    <label>Mot n° {n} <input autocomplete="off" bind:value={answers[i]} /></label>
  {/each}
  <button class="primary auth-submit" disabled={!ok} onclick={onDone}>Continuer</button>
</section>
