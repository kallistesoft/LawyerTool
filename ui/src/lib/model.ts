// TypeScript mirror of the Rust models (serde snake_case). Keep in sync with
// crates/applicant and crates/rules-engine; the backend refuses unknown fields.

export type MaritalStatus =
  | 'annulled' | 'common_law' | 'divorced' | 'legally_separated' | 'married' | 'never_married' | 'widowed'
export type EducationLevel =
  | 'less_than_secondary' | 'secondary' | 'one_year_post_secondary' | 'two_year_post_secondary'
  | 'bachelor_or_three_year' | 'two_or_more_credentials' | 'master_or_professional' | 'doctorate'
export type CanadianEducation = 'secondary_or_less' | 'one_or_two_year' | 'three_year_or_more'
export type TestKind = 'celpip_general' | 'ielts_general' | 'pte_core' | 'tef_canada' | 'tcf_canada'
export type Credential = 'canadian' | 'foreign_with_eca' | 'foreign_without_eca'

export interface LanguageTest {
  test: TestKind
  test_date: string
  speaking: number
  listening: number
  reading: number
  writing: number
}

export interface Spouse {
  is_canadian_citizen_or_pr: boolean
  accompanying: boolean
  education: EducationLevel | null
  canadian_work_years: number
  language: LanguageTest | null
}

export interface NocMonths { noc: string; months: number }

export interface WorkPeriod {
  noc: string
  in_canada: boolean
  start: string
  end: string
  hours_per_week: number
  paid: boolean
  self_employed: boolean
  full_time_student: boolean
  authorized: boolean
  qualified_in_trade: boolean | null
}

export interface EligibilityFacts {
  work_history: WorkPeriod[]
  credential: Credential | null
  canadian_study_two_years: boolean | null
  spouse_canadian_study_two_years: boolean | null
  spouse_canadian_work_one_year: boolean | null
  relative_in_canada: boolean | null
  authorized_to_work_in_canada: boolean | null
  fsw_arranged_employment: boolean | null
  intends_to_live_in_quebec: boolean | null
  settlement_funds_cad: number | null
  family_size: number | null
}

export interface Applicant {
  marital_status: MaritalStatus
  spouse: Spouse | null
  age: number
  education: EducationLevel
  canadian_education: CanadianEducation | null
  first_language: LanguageTest | null
  second_language: LanguageTest | null
  canadian_work_years: number
  canadian_work_by_noc: NocMonths[]
  foreign_work_years: number
  certificate_of_qualification: boolean
  job_offer: { noc: string } | null
  provincial_nomination: boolean
  sibling_in_canada: boolean
  eligibility: EligibilityFacts
}

export interface Identity {
  full_name: string
  email: string | null
  phone: string | null
  file_reference: string | null
}

export interface ClientSummary { id: number; full_name: string; file_reference: string | null; updated_at: string }
export interface ClientView {
  id: number
  created_at: string
  updated_at: string
  identity: Identity
  profile: Applicant
  /** To complete or check after a form import, until cleared. */
  notes: string[]
}

// --- results ---
export interface Cite { source: string; table: number | null; row: number | null; text: string }
export interface Line { label: string; points: number; cites: Cite[] }
export interface Section { lines: Line[]; subtotal: number }
export type Flag =
  | { ExpiredLanguageTest: { which: string } }
  | { KnownDivergence: { id: string } }
  | { ManualReview: { id: string; reason: string } }
export interface CrsResult {
  grid_id: string
  assessed_on: string
  with_spouse: boolean
  core: Section
  spouse: Section
  transferability: Section
  additional: Section
  labour_market: Section | null
  total: number
  flags: Flag[]
}
export type Check = 'Met' | 'NotMet' | 'NeedsInput' | 'LawyerCheck'
export interface Requirement { id: string; check: Check; detail: string; cites: Cite[] }
export interface FswPoints { lines: [string, number, number][]; min: number; max: number; pass_mark: number }
export interface ProgramResult {
  program: 'Fsw' | 'Cec' | 'Fst'
  status: 'Eligible' | 'NotEligible' | 'NeedsInput'
  requirements: Requirement[]
  fsw_points: FswPoints | null
}
export type Outcome<T> = { Ok: T } | { Err: string }
export interface Assessment {
  on: string
  grid_id: string | null
  crs: Outcome<CrsResult>
  eligibility: Outcome<ProgramResult[]>
  draws: { number: string; date: string; name: string; invitations: number; min_crs: number }[]
}

// --- labels (French UI) ---
export const MARITAL: Record<MaritalStatus, string> = {
  never_married: 'Jamais marié(e) / célibataire', married: 'Marié(e)', common_law: 'Conjoint(e) de fait',
  divorced: 'Divorcé(e) / séparé(e)', legally_separated: 'Séparé(e) légalement', widowed: 'Veuf / veuve',
  annulled: 'Mariage annulé',
}
export const EDUCATION: Record<EducationLevel, string> = {
  less_than_secondary: 'Études secondaires non complétées', secondary: 'Diplôme d’études secondaires',
  one_year_post_secondary: 'Programme postsecondaire d’un an', two_year_post_secondary: 'Programme postsecondaire de deux ans',
  bachelor_or_three_year: 'Baccalauréat ou programme de 3 ans et plus', two_or_more_credentials: 'Deux diplômes ou plus (dont un de 3 ans et plus)',
  master_or_professional: 'Maîtrise ou diplôme professionnel', doctorate: 'Doctorat (Ph. D.)',
}
export const CDN_EDUCATION: Record<CanadianEducation, string> = {
  secondary_or_less: 'Secondaire ou moins', one_or_two_year: 'Titre d’un ou deux ans', three_year_or_more: 'Titre de trois ans ou plus',
}
export const TESTS: Record<TestKind, string> = {
  celpip_general: 'CELPIP-G (anglais)', ielts_general: 'IELTS General Training (anglais)', pte_core: 'PTE Core (anglais)',
  tef_canada: 'TEF Canada (français)', tcf_canada: 'TCF Canada (français)',
}
export const CREDENTIAL: Record<Credential, string> = {
  canadian: 'Diplôme canadien', foreign_with_eca: 'Diplôme étranger avec EDE', foreign_without_eca: 'Diplôme étranger sans EDE',
}
export const PROGRAM: Record<ProgramResult['program'], string> = {
  Fsw: 'Travailleurs qualifiés (fédéral)', Cec: 'Expérience canadienne', Fst: 'Métiers spécialisés (fédéral)',
}

/** Today's date (YYYY-MM-DD) in the computer's time zone (not UTC). */
export function localToday(): string {
  const d = new Date()
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
}

/** IELTS bands are stored in tenths (6.5 → 65). */
export const isIelts = (t: TestKind) => t === 'ielts_general'

export function newLanguageTest(test: TestKind = 'ielts_general'): LanguageTest {
  return { test, test_date: localToday(), speaking: 0, listening: 0, reading: 0, writing: 0 }
}

export function newApplicant(): Applicant {
  return {
    marital_status: 'never_married', spouse: null, age: 30, education: 'bachelor_or_three_year', canadian_education: null,
    first_language: newLanguageTest(), second_language: null, canadian_work_years: 0, canadian_work_by_noc: [],
    foreign_work_years: 0, certificate_of_qualification: false, job_offer: null, provincial_nomination: false,
    sibling_in_canada: false,
    eligibility: {
      work_history: [], credential: null, canadian_study_two_years: null, spouse_canadian_study_two_years: null,
      spouse_canadian_work_one_year: null, relative_in_canada: null, authorized_to_work_in_canada: null,
      fsw_arranged_employment: null, intends_to_live_in_quebec: null, settlement_funds_cad: null, family_size: null,
    },
  }
}

export type ProgramStatus = ProgramResult['status']
/** Short program names for badges. */
export const PROGRAM_SHORT: Record<ProgramResult['program'], string> = { Fsw: 'TQF', Cec: 'CEC', Fst: 'TMS' }
export const STATUS_LABEL: Record<ProgramStatus, string> = {
  Eligible: 'Admissible', NotEligible: 'Non admissible', NeedsInput: 'À compléter',
}
export const STATUS_TONE: Record<ProgramStatus, 'ok' | 'err' | 'warn'> = { Eligible: 'ok', NotEligible: 'err', NeedsInput: 'warn' }

export interface ClientOverview {
  id: number
  full_name: string
  file_reference: string | null
  updated_at: string
  crs: number | null
  crs_error: string | null
  programs: [ProgramResult['program'], ProgramStatus][]
  reviews: number
  problems: number
}
