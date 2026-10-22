// Files on the lawyer's computer, through the browser's File System Access
// API (Chrome / Edge): the vault file and the two folders chosen in
// Paramètres. Handles are remembered in IndexedDB (this browser only); the
// browser asks the lawyer to confirm access again in each new session.

/// <reference path="./fs-access.d.ts" />

const DB = 'lawyertool'
const STORE = 'handles'

export type HandleKey = 'vault' | 'input' | 'output'

function db(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const r = indexedDB.open(DB, 1)
    r.onupgradeneeded = () => r.result.createObjectStore(STORE)
    r.onsuccess = () => resolve(r.result)
    r.onerror = () => reject(r.error)
  })
}

async function tx<T>(mode: IDBTransactionMode, f: (s: IDBObjectStore) => IDBRequest<T>): Promise<T> {
  const d = await db()
  return new Promise((resolve, reject) => {
    const r = f(d.transaction(STORE, mode).objectStore(STORE))
    r.onsuccess = () => resolve(r.result)
    r.onerror = () => reject(r.error)
  })
}

export const getHandle = <T extends FileSystemHandle>(k: HandleKey) =>
  tx<T | undefined>('readonly', (s) => s.get(k) as IDBRequest<T | undefined>)
export const setHandle = (k: HandleKey, h: FileSystemHandle) => tx('readwrite', (s) => s.put(h, k))
export const deleteHandle = (k: HandleKey) => tx('readwrite', (s) => s.delete(k))

export const supported = () =>
  typeof window !== 'undefined' && 'showOpenFilePicker' in window && 'showDirectoryPicker' in window

/** Current permission, without prompting. */
export async function permission(h: FileSystemHandle, mode: FileSystemPermissionMode): Promise<PermissionState> {
  return (await h.queryPermission?.({ mode })) ?? 'granted'
}

/** Asks for access if needed (must run during a click). */
export async function ensurePermission(h: FileSystemHandle, mode: FileSystemPermissionMode): Promise<boolean> {
  if ((await permission(h, mode)) === 'granted') return true
  return (await h.requestPermission?.({ mode })) === 'granted'
}

export const isAbort = (e: unknown) => e instanceof DOMException && e.name === 'AbortError'

const VAULT_TYPES: FilePickerAcceptType[] = [
  { description: 'Coffre LawyerTool', accept: { 'application/octet-stream': ['.ltvault'] } },
]

/** "Save as" for a new vault file; `null` if cancelled. */
export async function pickNewVault(): Promise<FileSystemFileHandle | null> {
  try {
    return await window.showSaveFilePicker({ suggestedName: 'Clients.ltvault', types: VAULT_TYPES, id: 'vault' })
  } catch (e) {
    if (isAbort(e)) return null
    throw e
  }
}

/** Opens an existing vault file; `null` if cancelled. */
export async function pickVault(): Promise<FileSystemFileHandle | null> {
  try {
    const [h] = await window.showOpenFilePicker({ types: VAULT_TYPES, id: 'vault', multiple: false })
    return h ?? null
  } catch (e) {
    if (isAbort(e)) return null
    throw e
  }
}

export async function pickFolder(which: 'input' | 'output'): Promise<FileSystemDirectoryHandle | null> {
  try {
    return await window.showDirectoryPicker({ id: which, mode: which === 'output' ? 'readwrite' : 'read' })
  } catch (e) {
    if (isAbort(e)) return null
    throw e
  }
}

export async function readFile(h: FileSystemFileHandle): Promise<Uint8Array> {
  return new Uint8Array(await (await h.getFile()).arrayBuffer())
}

/** Replaces the file's content (the browser writes a temporary copy, then swaps it in). */
export async function writeFile(h: FileSystemFileHandle, bytes: Uint8Array): Promise<void> {
  const w = await h.createWritable()
  try {
    await w.write(bytes as Uint8Array<ArrayBuffer>)
    await w.close()
  } catch (e) {
    await w.abort().catch(() => {})
    throw e
  }
}

async function exists(dir: FileSystemDirectoryHandle, name: string): Promise<boolean> {
  try {
    await dir.getFileHandle(name)
    return true
  } catch {
    return false
  }
}

/** Writes a new file in `dir`: `name`, or `stem-2.ext`, `-3`… if taken (never overwrites). Returns the name used. */
export async function writeNew(dir: FileSystemDirectoryHandle, name: string, bytes: Uint8Array): Promise<string> {
  const dot = name.lastIndexOf('.')
  const [stem, ext] = dot > 0 ? [name.slice(0, dot), name.slice(dot)] : [name, '']
  for (let n = 1; n < 1000; n++) {
    const candidate = n === 1 ? name : `${stem}-${n}${ext}`
    if (await exists(dir, candidate)) continue
    await writeFile(await dir.getFileHandle(candidate, { create: true }), bytes)
    return candidate
  }
  throw new Error('no free file name')
}

/** Whether the folder can still be read (moved or deleted folders throw). */
export async function folderExists(dir: FileSystemDirectoryHandle): Promise<boolean> {
  try {
    await dir.values().next()
    return true
  } catch {
    return false
  }
}

const DEV_VAULT = 'lawyertool-dev.ltvault'

/** Development only: the automatic test vault lives in the browser's private storage. */
export async function devVaultHandle(): Promise<FileSystemFileHandle> {
  const root = await navigator.storage.getDirectory()
  return root.getFileHandle(DEV_VAULT, { create: true })
}

/** Development only: erases this browser's private storage (test vault) and the remembered handles. */
export async function eraseBrowserData(): Promise<void> {
  const root = await navigator.storage.getDirectory()
  for await (const entry of root.values()) await root.removeEntry(entry.name, { recursive: true })
  await new Promise<void>((resolve, reject) => {
    const r = indexedDB.deleteDatabase(DB)
    r.onsuccess = () => resolve()
    r.onerror = () => reject(r.error)
    r.onblocked = () => resolve()
  })
}
