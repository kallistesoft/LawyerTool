// The only bridge to the backend. The frontend never talks to the network
// and never sees keys or secrets; every call goes through a Rust command.
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

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
}

/** Error shape returned by every command. */
export interface CommandError {
  code: string
  message: string
}

export const errorMessage = (e: unknown): string =>
  typeof e === 'object' && e !== null && 'message' in e ? String((e as CommandError).message) : String(e)

export const appInfo = () => invoke<AppInfo>('app_info')
export const vaultStatus = () => invoke<VaultStatus>('vault_status')
export const vaultCreate = (passphrase: string) =>
  invoke<{ recovery_phrase: string }>('vault_create', { passphrase })
export const vaultUnlock = (passphrase: string) => invoke<void>('vault_unlock', { passphrase })
export const vaultRecover = (phrase: string, newPassphrase: string) =>
  invoke<void>('vault_recover', { phrase, newPassphrase })
export const vaultLock = () => invoke<void>('vault_lock')

/** Reports user activity, at most every 30 s, to defer auto-lock. */
let lastActivity = 0
export function reportActivity() {
  const now = Date.now()
  if (now - lastActivity > 30_000) {
    lastActivity = now
    invoke('activity').catch(() => {})
  }
}

export const onLocked = (f: () => void): Promise<UnlistenFn> => listen('vault-locked', f)

export interface UpdateInfo {
  configured: boolean
  available: boolean
  current: string
  version: string | null
  notes: string | null
  /** Current version is below the release's min_version. */
  mandatory: boolean
  channel: 'stable' | 'beta'
}

export const updateCheck = () => invoke<UpdateInfo>('update_check')
/** Downloads, verifies and installs, then restarts the app. */
export const updateInstall = () => invoke<void>('update_install')
