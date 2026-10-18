// The only bridge to the backend. The frontend never talks to the network
// and never sees keys or secrets; every call goes through a Rust command.
import { invoke } from '@tauri-apps/api/core'

export interface AppInfo {
  name: string
  version: string
}

export const appInfo = () => invoke<AppInfo>('app_info')
