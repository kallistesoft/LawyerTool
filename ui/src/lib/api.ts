// The only bridge to the engine. The engine (Rust compiled to WebAssembly)
// runs in a Web Worker and holds the unlocked vault; this module reads and
// writes the vault file and the chosen folders on the lawyer's computer.
// Client data never leaves the computer: the only network request is the
// public rules snapshot (watch/latest.json) from this site's own origin.
import {
  deleteHandle, devVaultHandle, ensurePermission, eraseBrowserData, folderExists, getHandle, isAbort, permission, pickFolder,
  pickNewVault, pickVault, readFile, setHandle, writeFile, writeNew,
} from './files'
import type { Op, Reply } from './engine.worker'
import type { Applicant, Assessment, ClientOverview, ClientSummary, ClientView, Identity } from './model'

export interface AppInfo {
  name: string
  version: string
}

export interface VaultStatus {
  exists: boolean
  unlocked: boolean
  idle_minutes: number
  /** Dev build with the automatic test vault (no passphrase, no auto-lock). */
  dev: boolean
  /** Name of the vault file (not its path: the browser does not reveal it). */
  file: string | null
}

/** Error shape returned by every command. */
export interface CommandError {
  code: string
  message: string
}

export const errorMessage = (e: unknown): string =>
  typeof e === 'object' && e !== null && 'message' in e ? String((e as CommandError).message) : String(e)

const fail = (code: string, message: string): CommandError => ({ code, message })

// ---- Engine worker -------------------------------------------------------

const worker = new Worker(new URL('./engine.worker.ts', import.meta.url), { type: 'module' })
let nextId = 1
const waiting = new Map<number, (r: Reply) => void>()
worker.onmessage = (ev: MessageEvent<Reply>) => {
  waiting.get(ev.data.id)?.(ev.data)
  waiting.delete(ev.data.id)
}
// A crashed engine must not leave the page waiting forever.
worker.onerror = () => {
  for (const [id, resolve] of waiting) resolve({ id, ok: false, value: fail('internal', 'Erreur interne du moteur : rechargez la page.') })
  waiting.clear()
}

/** Vault writes, one at a time and in order. */
let saving: Promise<void> = Promise.resolve()

async function engine<T>(m: Op, transfer: Transferable[] = []): Promise<T> {
  const id = nextId++
  const reply = await new Promise<Reply>((resolve) => {
    waiting.set(id, resolve)
    worker.postMessage({ id, ...m }, transfer)
  })
  if (!reply.ok) throw reply.value
  if (reply.sealed) await save(reply.sealed)
  return reply.value as T
}

const call = <T>(cmd: string, args?: unknown) => engine<T>({ op: 'call', cmd, args })

let vaultHandle: FileSystemFileHandle | null = null

function save(bytes: Uint8Array): Promise<void> {
  const h = vaultHandle
  const p = saving.then(async () => {
    if (!h) throw fail('save_failed', 'Aucun fichier de coffre.')
    try {
      await writeFile(h, bytes)
    } catch {
      throw fail('save_failed', 'Le coffre n’a pas pu être enregistré : vérifiez que le fichier est toujours accessible.')
    }
  })
  saving = p.catch(() => {})
  return p
}

// ---- Vault ---------------------------------------------------------------

const DEV = __DEV_VAULT__ && !new URLSearchParams(location.search).has('real')
const DEV_PASSPHRASE = 'dev-only-not-a-secret'
const IDLE_MS = 5 * 60_000

export const appInfo = async (): Promise<AppInfo> => ({ name: 'LawyerTool', version: __APP_VERSION__ })

async function rememberedVault(): Promise<FileSystemFileHandle | null> {
  if (DEV) return devVaultHandle()
  return (await getHandle<FileSystemFileHandle>('vault')) ?? null
}

/** Dev builds: creates the test vault on first run, then unlocks it. */
async function openDevVault() {
  vaultHandle = await devVaultHandle()
  const file = await readFile(vaultHandle)
  if (file.length === 0) await engine({ op: 'create', passphrase: DEV_PASSPHRASE })
  else await engine({ op: 'unlock', file, passphrase: DEV_PASSPHRASE }, [file.buffer])
  touch()
}

export async function vaultStatus(): Promise<VaultStatus> {
  let s = await engine<{ unlocked: boolean }>({ op: 'status' })
  if (DEV && !s.unlocked) {
    await openDevVault()
    s = await engine({ op: 'status' })
  }
  const h = vaultHandle ?? (await rememberedVault())
  return { exists: h !== null, unlocked: s.unlocked, idle_minutes: IDLE_MS / 60_000, dev: DEV, file: h?.name ?? null }
}

/** Opens the "Save as" dialog for the new vault file, then creates it. `null` if cancelled. */
export async function vaultCreate(passphrase: string): Promise<{ recovery_phrase: string } | null> {
  const h = await pickNewVault()
  if (!h) return null
  vaultHandle = h
  const created = await engine<{ recovery_phrase: string }>({ op: 'create', passphrase })
  await setHandle('vault', h)
  touch()
  return created
}

/** Lets the lawyer pick an existing vault file (another computer, a backup). */
export async function vaultChoose(): Promise<boolean> {
  const h = await pickVault()
  if (!h) return false
  await setHandle('vault', h)
  vaultHandle = h
  return true
}

/** Reads the vault file, asking the browser for access if needed (call on a click). */
async function vaultBytes(): Promise<Uint8Array> {
  const h = vaultHandle ?? (await rememberedVault())
  if (!h) throw fail('not_found', 'Aucun coffre n’est choisi.')
  if (!(await ensurePermission(h, 'readwrite')))
    throw fail('permission', 'L’accès au fichier du coffre a été refusé.')
  vaultHandle = h
  try {
    return await readFile(h)
  } catch {
    throw fail('not_found', 'Le fichier du coffre est introuvable : ouvrez-le de nouveau.')
  }
}

export async function vaultUnlock(passphrase: string): Promise<void> {
  const file = await vaultBytes()
  await engine({ op: 'unlock', file, passphrase }, [file.buffer])
  touch()
}

export async function vaultRecover(phrase: string, newPassphrase: string): Promise<void> {
  const file = await vaultBytes()
  await engine({ op: 'recover', file, phrase, newPassphrase }, [file.buffer])
  touch()
}

type Listener = () => void
type UnlistenFn = () => void
const listeners: Record<'locked' | 'watch', Set<Listener>> = { locked: new Set(), watch: new Set() }
const emit = (k: keyof typeof listeners) => listeners[k].forEach((f) => f())
const listen = (k: keyof typeof listeners, f: Listener): Promise<UnlistenFn> => {
  listeners[k].add(f)
  return Promise.resolve(() => listeners[k].delete(f))
}

export async function vaultLock(): Promise<void> {
  await saving
  if (await engine<boolean>({ op: 'lock' })) emit('locked')
}

export const onLocked = (f: () => void): Promise<UnlistenFn> => listen('locked', f)

/** Dev / demo build (test vault, no passphrase). */
export const devMode = DEV

/**
 * Dev / demo only: erases every client, setting and watcher result (the
 * test vault in this browser) and the remembered folders, then reloads with
 * an empty test vault. Files in the lawyer's folders are not touched.
 */
export async function devEraseAll(): Promise<void> {
  if (!DEV) throw fail('forbidden', 'Réservé à la version de démonstration.')
  await saving
  await engine({ op: 'lock' })
  vaultHandle = null
  await eraseBrowserData()
  location.reload()
}

// Auto-lock after 5 minutes without activity, or when the computer slept
// (the clock jumped between two ticks). Never in the dev build.
let lastActivity = Date.now()
const touch = () => (lastActivity = Date.now())
export const reportActivity = touch
const TICK_MS = 5_000
const SLEEP_GAP_MS = 30_000
let lastTick = Date.now()
setInterval(async () => {
  const now = Date.now()
  const slept = now - lastTick > SLEEP_GAP_MS || now < lastTick
  lastTick = now
  if (DEV) return
  if (slept || now - lastActivity >= IDLE_MS) {
    const { unlocked } = await engine<{ unlocked: boolean }>({ op: 'status' })
    if (unlocked) await vaultLock()
  }
}, TICK_MS)

// ---- Clients -------------------------------------------------------------

export const clientList = () => call<ClientSummary[]>('client_list')
/** Every client with score, program statuses and pending checks (computed on today's grid). */
export const clientOverview = (on: string) => call<ClientOverview[]>('client_overview', { on })
export const clientGet = (id: number) => call<ClientView>('client_get', { id })
export const clientSave = (id: number | null, identity: Identity, profile: Applicant) =>
  call<{ id: number; problems: string[] }>('client_save', { request: { id, identity, profile } })
export const clientDelete = (id: number) => call<boolean>('client_delete', { id })
/** The lawyer has completed what the form import left to check. */
export const clientClearNotes = (id: number) => call<void>('client_clear_notes', { id })
export const assess = (id: number, on: string | null) => call<Assessment>('assess', { id, on })

export interface Redaction { path: string; kind: string; placeholder: string }
/** The exact (de-identified) JSON that would be sent to Claude. Sends nothing. */
export const deidPreview = (id: number, on: string | null) =>
  call<{ payload: string; bytes: number; redactions: Redaction[] }>('deid_preview', { id, on })

// ---- Folders (Paramètres) -------------------------------------------------

export interface FolderStatus {
  /** Folder name (the browser does not reveal full paths). */
  path: string
  exists: boolean
  writable: boolean
  /** The browser needs the lawyer to confirm access again (new session). */
  needs_permission: boolean
}
export interface Settings { input_dir: FolderStatus | null; output_dir: FolderStatus | null }
export type Folder = 'input' | 'output'

async function folderStatus(which: Folder): Promise<FolderStatus | null> {
  const h = await getHandle<FileSystemDirectoryHandle>(which)
  if (!h) return null
  const mode = which === 'output' ? 'readwrite' : 'read'
  const granted = (await permission(h, mode)) === 'granted'
  const exists = granted ? await folderExists(h) : true
  return { path: h.name, exists, writable: which === 'output' && granted && exists, needs_permission: !granted }
}

export const settingsGet = async (): Promise<Settings> => ({
  input_dir: await folderStatus('input'),
  output_dir: await folderStatus('output'),
})

/** Opens the folder dialog and remembers the choice (unchanged if cancelled). */
export async function settingsPick(which: Folder): Promise<Settings> {
  const h = await pickFolder(which)
  if (h) await setHandle(which, h)
  return settingsGet()
}

export async function settingsClear(which: Folder): Promise<Settings> {
  await deleteHandle(which)
  return settingsGet()
}

/** Asks the browser for access to both folders again (call on a click). */
export async function folderAccess(): Promise<Settings> {
  for (const which of ['input', 'output'] as const) {
    const h = await getHandle<FileSystemDirectoryHandle>(which)
    if (h) await ensurePermission(h, which === 'output' ? 'readwrite' : 'read')
  }
  return settingsGet()
}

// ---- Export ----------------------------------------------------------------

/**
 * Saves the report in the output folder (Paramètres) or, if none is set,
 * where the lawyer chooses. Refused until every review flag is checked.
 * Returns the file name, or null if the dialog was cancelled.
 */
export async function exportReport(id: number, on: string | null, format: 'pdf' | 'docx', reviewed: string[]) {
  const dir = await getHandle<FileSystemDirectoryHandle>('output')
  if (dir && !(await ensurePermission(dir, 'readwrite')))
    throw fail('permission', 'L’accès au dossier de sortie a été refusé.')
  const f = await engine<{ name: string; mime: string; kind: string; bytes: Uint8Array }>({
    op: 'export',
    request: { id, on, format, reviewed },
  })
  if (dir) {
    if (!(await folderExists(dir))) throw fail('output_dir_missing', 'Le dossier de sortie est introuvable : vérifiez Paramètres.')
    try {
      return await writeNew(dir, f.name, f.bytes)
    } catch {
      throw fail('write_failed', 'Le fichier n’a pas pu être enregistré.')
    }
  }
  try {
    const h = await window.showSaveFilePicker({
      suggestedName: f.name,
      types: [{ description: f.kind, accept: { [f.mime]: [`.${format}`] } }],
    })
    await writeFile(h, f.bytes)
    return h.name
  } catch (e) {
    if (isAbort(e)) return null
    throw fail('write_failed', 'Le fichier n’a pas pu être enregistré.')
  }
}

// ---- Inbox (input folder) ----------------------------------------------------

export interface InboxFile {
  name: string
  size: number
  modified: string | null
  status: 'new' | 'imported' | 'failed' | 'too_large'
  message: string | null
  client_id: number | null
}
export interface ImportResult {
  name: string
  ok: boolean
  client_id: number | null
  message: string
  /** Every client of the file's responses, new or imported before. */
  client_ids: number[]
}

const MAX_FILE = 10 * 1024 * 1024
const MAX_ENTRIES = 500
const accepted = (n: string) => /\.(xlsx|csv)$/i.test(n) && !n.startsWith('.') && !n.startsWith('~$')
const hex = (b: ArrayBuffer) => [...new Uint8Array(b)].map((x) => x.toString(16).padStart(2, '0')).join('')

async function inputDir(): Promise<FileSystemDirectoryHandle> {
  const dir = await getHandle<FileSystemDirectoryHandle>('input')
  if (!dir) throw fail('no_input_dir', 'Choisissez d’abord le dossier des formulaires dans Paramètres.')
  if ((await permission(dir, 'read')) !== 'granted')
    throw fail('needs_permission', 'Le navigateur demande de confirmer l’accès au dossier des formulaires.')
  if (!(await folderExists(dir))) throw fail('input_dir_missing', 'Le dossier des formulaires est introuvable.')
  return dir
}

/** Lists the input folder (one level, plain .xlsx/.csv files, read only). */
export async function inboxList(): Promise<InboxFile[]> {
  const dir = await inputDir()
  const files = []
  try {
    for await (const entry of dir.values()) {
      if (entry.kind !== 'file' || !accepted(entry.name)) continue
      const file = await entry.getFile()
      const fingerprint = file.size <= MAX_FILE ? hex(await crypto.subtle.digest('SHA-256', await file.arrayBuffer())) : null
      files.push({ name: entry.name, size: file.size, modified: new Date(file.lastModified).toISOString(), fingerprint })
      if (files.length >= MAX_ENTRIES) break
    }
  } catch {
    throw fail('read_failed', 'Le dossier des formulaires n’a pas pu être lu.')
  }
  return call<InboxFile[]>('inbox_status', { files })
}

// ---- Audits (Word) -----------------------------------------------------------

/** What the lawyer sets before generating an audit (see crates/audit-doc). */
export interface AuditLawyer {
  noc: string | null
  noc_validated: boolean
  spouse_noc: string | null
  spouse_noc_validated: boolean
  regulated: string | null
  note: string | null
}
export interface AuditInputs {
  lawyer: AuditLawyer
  suggested_noc: string | null
  suggested_spouse_noc: string | null
  married: boolean
  job: string
  spouse_job: string
}
/** `null` when the client was not imported from the form. */
export const auditInputsGet = (id: number) => call<AuditInputs | null>('audit_inputs_get', { id })
export const auditInputsSet = (id: number, lawyer: AuditLawyer) => call<void>('audit_inputs_set', { id, lawyer })

export interface AuditResult { client_id: number; ok: boolean; file: string | null; message: string }

/**
 * « Générer les audits »: imports the form files of the input folder (new
 * responses become clients), then writes one Word audit per response into
 * the output folder (never overwriting: -2, -3…). Call on a click.
 */
export async function generateAudits(): Promise<AuditResult[]> {
  const out = await getHandle<FileSystemDirectoryHandle>('output')
  if (!out) throw fail('no_output_dir', 'Choisissez d’abord le dossier de sortie dans Paramètres.')
  const input = await getHandle<FileSystemDirectoryHandle>('input')
  if (input && !(await ensurePermission(input, 'read')))
    throw fail('permission', 'L’accès au dossier des formulaires a été refusé.')
  if (!(await ensurePermission(out, 'readwrite'))) throw fail('permission', 'L’accès au dossier de sortie a été refusé.')
  if (!(await folderExists(out))) throw fail('output_dir_missing', 'Le dossier de sortie est introuvable : vérifiez Paramètres.')
  const files = await inboxList()
  const imported = await inboxImport(files.filter((f) => f.status !== 'too_large').map((f) => f.name))
  const ids = [...new Set(imported.flatMap((r) => r.client_ids))]
  const results: AuditResult[] = []
  for (const id of ids) {
    try {
      const f = await engine<{ name: string; bytes: Uint8Array }>({ op: 'audit', client: id })
      const file = await writeNew(out, f.name, f.bytes)
      results.push({ client_id: id, ok: true, file, message: 'Audit enregistré.' })
    } catch (e) {
      results.push({ client_id: id, ok: false, file: null, message: errorMessage(e) })
    }
  }
  return results
}

/** Imports the named files from the input folder (skipping those already imported). */
export async function inboxImport(names: string[]): Promise<ImportResult[]> {
  const dir = await inputDir()
  const results: ImportResult[] = []
  for (const name of names.slice(0, MAX_ENTRIES)) {
    let bytes: Uint8Array
    try {
      bytes = new Uint8Array(await (await (await dir.getFileHandle(name)).getFile()).arrayBuffer())
    } catch {
      results.push({ name, ok: false, client_id: null, message: 'Fichier illisible.', client_ids: [] })
      continue
    }
    results.push(await engine<ImportResult>({ op: 'inbox_import', name, bytes }, [bytes.buffer]))
  }
  return results
}

// ---- Rules watcher ---------------------------------------------------------

export interface DrawRow { number: string; date: string; name: string; invitations: number; min_crs: number; first_seen: string }
export interface WatchRun { at: string; ok: boolean; summary: string }
export interface GridChange { path: string; before: string | null; after: string | null; source_text: string | null }
export type SentenceChange = { added: string } | { removed: string }
export interface Pending {
  base_grid: string
  changes: GridChange[]
  part_errors: string[]
  prose: { source: string; changes: SentenceChange[] }[]
  alerts: string[]
}
export interface WatchStatus {
  last_run: WatchRun | null
  last_ok: WatchRun | null
  stale: boolean
  running: boolean
  pending: [string, Pending] | null
  draws: DrawRow[]
  grids: [string, string][]
}

let watchRunning = false

export const watchStatus = async () => ({ ...(await call<WatchStatus>('watch_status')), running: watchRunning })

/** Fetches the rules snapshot the daily job publishes on this site, and compares it with the vault. */
export async function watchCheckNow(): Promise<unknown> {
  if (watchRunning) throw fail('watch_failed', 'Une vérification est déjà en cours.')
  watchRunning = true
  emit('watch')
  const host = location.host
  try {
    let res: Response
    try {
      res = await fetch(`${import.meta.env.BASE_URL}watch/latest.json`, { cache: 'no-store', credentials: 'same-origin' })
    } catch {
      await call('watch_failed', { fetch: { host, status: null, bytes: 0 }, reason: 'site injoignable' })
      throw fail('watch_failed', 'La veille publiée n’a pas pu être téléchargée.')
    }
    const text = await res.text()
    const fetchInfo = { host, status: res.status, bytes: text.length }
    if (!res.ok) {
      await call('watch_failed', { fetch: fetchInfo, reason: `HTTP ${res.status}` })
      throw fail('watch_failed', `La veille publiée est indisponible (HTTP ${res.status}).`)
    }
    let published: unknown
    try {
      published = JSON.parse(text)
    } catch {
      await call('watch_failed', { fetch: fetchInfo, reason: 'fichier illisible' })
      throw fail('watch_failed', 'La veille publiée est illisible.')
    }
    return await call('watch_apply', { fetch: fetchInfo, published })
  } finally {
    watchRunning = false
    emit('watch')
  }
}

export const watchApprove = (note: string | null) => call<string[]>('watch_approve', { note })
export const watchReject = () => call<void>('watch_reject')
export const onWatchUpdated = (f: () => void): Promise<UnlistenFn> => listen('watch', f)

// Daily check while unlocked: right after unlock, then once a day.
setInterval(async () => {
  try {
    const { unlocked } = await engine<{ unlocked: boolean }>({ op: 'status' })
    if (unlocked && !watchRunning && (await call<boolean>('watch_due'))) await watchCheckNow()
  } catch {
    // Recorded in the vault as a failed run; shown in "Veille des règles".
  }
}, 60_000)
