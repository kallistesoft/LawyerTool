<script lang="ts">
  import { isIelts, TESTS, type LanguageTest, type TestKind } from './model'

  let { value = $bindable(), title, only }: { value: LanguageTest; title: string; only?: 'english' | 'french' } = $props()
  const skills = [
    ['speaking', 'Expression orale'],
    ['listening', 'Compréhension de l’oral'],
    ['reading', 'Compréhension de l’écrit'],
    ['writing', 'Expression écrite'],
  ] as const
  const french: TestKind[] = ['tef_canada', 'tcf_canada']
  let kinds = $derived(
    (Object.keys(TESTS) as TestKind[]).filter((k) =>
      only === undefined ? true : only === 'french' ? french.includes(k) : !french.includes(k),
    ),
  )
  // IELTS is stored in tenths; show and edit it as a band (6.5).
  const show = (n: number) => (isIelts(value.test) ? n / 10 : n)
  const store = (s: string) => {
    const n = Number(s.replace(',', '.'))
    return Number.isFinite(n) ? Math.round(isIelts(value.test) ? n * 10 : n) : 0
  }
  let hint = $derived(
    {
      ielts_general: 'Bandes 0–9 (ex. 6,5)',
      celpip_general: 'Niveaux 0–12',
      pte_core: '10–90',
      tef_canada: '0–699 (barème selon la date du test)',
      tcf_canada: 'Oral/écrit : 0–20 ; compréhension : 100–699',
    }[value.test],
  )
</script>

<fieldset>
  <legend>{title}</legend>
  <div class="row">
    <label>Test
      <select bind:value={value.test}>
        {#each kinds as k (k)}<option value={k}>{TESTS[k]}</option>{/each}
      </select>
    </label>
    <label>Date du test <input type="date" bind:value={value.test_date} /></label>
  </div>
  <p class="hint">{hint}</p>
  <div class="row">
    {#each skills as [key, label] (key)}
      <label>{label}
        <input inputmode="decimal" value={show(value[key])} onchange={(e) => (value[key] = store((e.currentTarget as HTMLInputElement).value))} />
      </label>
    {/each}
  </div>
</fieldset>
