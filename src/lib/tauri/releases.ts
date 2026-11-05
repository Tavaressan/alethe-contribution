import { Channel, invoke } from '@tauri-apps/api/core'
import type { DownloadEvent } from '@tauri-apps/plugin-updater'

export async function downloadRelease(
  version: string,
  onEvent: (event: DownloadEvent) => void,
): Promise<void> {
  const channel = new Channel<DownloadEvent>()
  channel.onmessage = onEvent
  await invoke('download_release', { version, onEvent: channel })
}

/** Who updates this install, or `null` when the app updates itself. Empty when unnamed. */
export async function packageManager(): Promise<string | null> {
  return invoke<string | null>('package_manager')
}

export async function installDownloadedRelease(): Promise<void> {
  await invoke('install_downloaded_release')
}
