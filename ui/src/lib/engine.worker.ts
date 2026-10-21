// Runs the engine (crates/web compiled to WebAssembly) off the main thread,
// so Argon2id and report rendering never freeze the page. Holds the unlocked
// vault in memory; after every change it returns the freshly sealed vault
// file, which the page writes back to disk.
import init, { Engine } from '../engine/engine.js'

export type Op =
  | { op: 'call'; cmd: string; args: unknown }
  | { op: 'create'; passphrase: string }
  | { op: 'unlock'; file: Uint8Array; passphrase: string }
  | { op: 'recover'; file: Uint8Array; phrase: string; newPassphrase: string }
  | { op: 'lock' }
  | { op: 'status' }
  | { op: 'export'; request: unknown }
  | { op: 'inbox_import'; name: string; bytes: Uint8Array }

export interface Reply {
  id: number
  ok: boolean
  value?: unknown
  /** New vault file content, when the vault changed. */
  sealed?: Uint8Array
}

const fastKdf = import.meta.env.DEV
let engine: Engine | null = null
const ready = init().then(() => (engine = new Engine(fastKdf)))

function run(e: Engine, m: Op): unknown {
  switch (m.op) {
    case 'call':
      return JSON.parse(e.call(m.cmd, m.args === undefined ? '' : JSON.stringify(m.args)))
    case 'create':
      return JSON.parse(e.create(m.passphrase))
    case 'unlock':
      return e.unlock(m.file, m.passphrase)
    case 'recover':
      return e.recover(m.file, m.phrase, m.newPassphrase)
    case 'lock':
      return e.lock()
    case 'status':
      return { unlocked: e.is_unlocked(), vault_id: e.vault_id() ?? null }
    case 'export': {
      const out = e.export_report(JSON.stringify(m.request))
      return { ...JSON.parse(out.meta), bytes: out.bytes }
    }
    case 'inbox_import':
      return JSON.parse(e.inbox_import(m.name, m.bytes))
  }
}

/** Engine errors are thrown as the JSON of `{ code, message }`. */
function asError(err: unknown): unknown {
  if (typeof err === 'string') {
    try {
      return JSON.parse(err)
    } catch {
      return { code: 'internal', message: err }
    }
  }
  return { code: 'internal', message: 'Erreur interne.' }
}

self.onmessage = async (ev: MessageEvent<{ id: number } & Op>) => {
  const { id } = ev.data
  await ready
  const e = engine!
  let reply: Reply
  try {
    reply = { id, ok: true, value: run(e, ev.data) }
    if (e.take_dirty()) reply.sealed = e.seal()
  } catch (err) {
    reply = { id, ok: false, value: asError(err) }
  }
  self.postMessage(reply, { transfer: reply.sealed ? [reply.sealed.buffer as ArrayBuffer] : [] })
}
