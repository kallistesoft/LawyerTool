<script lang="ts">
  let { value = $bindable(''), valid = $bindable(false), label = 'Phrase de passe' } = $props()
  let confirm = $state('')
  const MIN = 12
  let tooShort = $derived(value.length > 0 && [...value].length < MIN)
  let mismatch = $derived(confirm.length > 0 && confirm !== value)
  $effect(() => {
    valid = [...value].length >= MIN && value === confirm
  })
</script>

<label>
  {label}
  <input type="password" autocomplete="new-password" bind:value />
</label>
{#if tooShort}<p class="hint">Au moins {MIN} caractères. Une courte phrase de plusieurs mots est idéale.</p>{/if}
<label>
  Confirmer
  <input type="password" autocomplete="new-password" bind:value={confirm} />
</label>
{#if mismatch}<p class="hint">Les deux saisies ne correspondent pas.</p>{/if}
