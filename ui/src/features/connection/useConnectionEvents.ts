import { useEffect } from 'react'
import { listen } from '@tauri-apps/api/event'

import { useAppStore } from '../../store/appStore'
import type { ConnectionSnapshot, CredentialPrompt, DnsObservation, LogEntry } from '../../types/ipc'

export function useConnectionEvents() {
  const setConnection = useAppStore((state) => state.setConnection)
  const setDnsObservation = useAppStore((state) => state.setDnsObservation)
  const appendLog = useAppStore((state) => state.appendLog)
  const setCredentialPrompt = useAppStore((state) => state.setCredentialPrompt)

  useEffect(() => {
    const unlisten = Promise.all([
      listen<ConnectionSnapshot>('connection://state-changed', (event) => {
        setConnection(event.payload)
      }),
      listen<LogEntry>('connection://log-line', (event) => {
        appendLog(event.payload)
      }),
      listen<CredentialPrompt>('connection://credentials-requested', (event) => {
        setCredentialPrompt(event.payload)
      }),
      listen<DnsObservation>('connection://dns-observed', (event) => {
        setDnsObservation(event.payload)
      }),
    ])

    return () => {
      void unlisten.then((listeners) => {
        listeners.forEach((listener) => listener())
      })
    }
  }, [appendLog, setConnection, setCredentialPrompt, setDnsObservation])
}
