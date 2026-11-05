import { check } from '@tauri-apps/plugin-updater'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { downloadRelease, packageManager } from './tauri/releases'
import { checkForUpdate, installPendingUpdate, installRelease } from './updater'

const order = vi.hoisted(() => [] as string[])
const flush = vi.hoisted(() => vi.fn(async () => {}))

vi.mock('@tauri-apps/plugin-updater', () => ({
  check: vi.fn(async () => ({
    version: '9.9.9',
    currentVersion: '1.0.0',
    download: vi.fn(async () => {
      order.push('download')
    }),
    install: vi.fn(async () => {
      order.push('install')
    }),
  })),
}))
vi.mock('@tauri-apps/plugin-process', () => ({
  relaunch: vi.fn(async () => {
    order.push('relaunch')
  }),
}))
vi.mock('../stores/projectsStore', () => ({ flushProjectsState: flush }))
vi.mock('./tauri/releases', () => ({
  packageManager: vi.fn(async () => null),
  downloadRelease: vi.fn(async () => {}),
  installDownloadedRelease: vi.fn(async () => {}),
}))

beforeEach(() => {
  vi.mocked(check).mockClear()
  vi.mocked(downloadRelease).mockClear()
  vi.mocked(packageManager).mockReset()
  vi.mocked(packageManager).mockResolvedValue(null)
  order.length = 0
  flush.mockReset()
  flush.mockImplementation(async () => {
    order.push('flush')
  })
})

describe('installPendingUpdate', () => {
  it('saves the workspace before the installer takes the process down', async () => {
    await checkForUpdate()
    await installPendingUpdate()

    expect(order).toEqual(['download', 'flush', 'install', 'relaunch'])
  })

  it('still installs when the workspace cannot be saved', async () => {
    flush.mockRejectedValueOnce(new Error('disk full'))
    await checkForUpdate()
    await installPendingUpdate()

    expect(order).toEqual(['download', 'install', 'relaunch'])
  })
})

describe('an install owned by a package manager', () => {
  beforeEach(() => {
    vi.mocked(packageManager).mockResolvedValue('pacman')
  })

  it('never looks for updates', async () => {
    await expect(checkForUpdate()).resolves.toBeNull()
    expect(check).not.toHaveBeenCalled()
  })

  it('refuses to install the pending update', async () => {
    await expect(installPendingUpdate()).rejects.toThrow('managed_install')
    expect(check).not.toHaveBeenCalled()
    expect(order).toEqual([])
  })

  it('refuses to install a chosen release', async () => {
    await expect(installRelease('1.6.0')).rejects.toThrow('managed_install')
    expect(downloadRelease).not.toHaveBeenCalled()
  })

  it('treats a marker with no name the same way', async () => {
    vi.mocked(packageManager).mockResolvedValue('')
    await expect(checkForUpdate()).resolves.toBeNull()
    expect(check).not.toHaveBeenCalled()
  })
})
