import { useEffect } from 'react'
import { listen } from '@tauri-apps/api/event'

import { useAppStore } from '../../store/appStore'
import type { UpdateProgress } from '../../types/ipc'

/** Loads version/helper info, checks for an update once at startup and tracks download progress. */
export function useUpdateEvents() {
  const loadAppInfo = useAppStore((state) => state.loadAppInfo)
  const checkForUpdates = useAppStore((state) => state.checkForUpdates)
  const setUpdateProgress = useAppStore((state) => state.setUpdateProgress)

  useEffect(() => {
    void loadAppInfo()
    void checkForUpdates(false)
  }, [loadAppInfo, checkForUpdates])

  useEffect(() => {
    const unlisten = listen<UpdateProgress>('update://download-progress', (event) => {
      setUpdateProgress(event.payload)
    })
    return () => {
      void unlisten.then((listener) => listener())
    }
  }, [setUpdateProgress])
}
