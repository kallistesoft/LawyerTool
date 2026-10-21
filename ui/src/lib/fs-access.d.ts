// File System Access API parts that TypeScript's DOM library does not ship
// (Chromium-only: pickers and permissions).

type FileSystemPermissionMode = 'read' | 'readwrite'

interface FileSystemHandle {
  queryPermission?(d: { mode: FileSystemPermissionMode }): Promise<PermissionState>
  requestPermission?(d: { mode: FileSystemPermissionMode }): Promise<PermissionState>
}

interface FileSystemDirectoryHandle {
  values(): AsyncIterableIterator<FileSystemFileHandle | FileSystemDirectoryHandle>
}

interface FilePickerAcceptType {
  description?: string
  accept: Record<string, string[]>
}

interface Window {
  showOpenFilePicker(o?: { types?: FilePickerAcceptType[]; id?: string; multiple?: boolean }): Promise<FileSystemFileHandle[]>
  showSaveFilePicker(o?: { suggestedName?: string; types?: FilePickerAcceptType[]; id?: string }): Promise<FileSystemFileHandle>
  showDirectoryPicker(o?: { id?: string; mode?: FileSystemPermissionMode }): Promise<FileSystemDirectoryHandle>
}
