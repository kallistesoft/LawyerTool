<script lang="ts">
  // Yes / No / Unknown. Unknown (null) makes the matching requirement "needs input".
  let { value = $bindable(null), label }: { value: boolean | null; label: string } = $props()
  let s = $derived(value === null ? '' : value ? 'yes' : 'no')
</script>

<label class="inline">
  <span>{label}</span>
  <select value={s} onchange={(e) => {
    const v = (e.currentTarget as HTMLSelectElement).value
    value = v === '' ? null : v === 'yes'
  }}>
    <option value="">Inconnu</option>
    <option value="yes">Oui</option>
    <option value="no">Non</option>
  </select>
</label>
