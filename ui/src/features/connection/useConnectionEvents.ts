import { useEffect, useRef } from 'react'
import { listen } from '@tauri-apps/api/event'

import { useAppStore } from '../../store/appStore'
import type { ConnectionSnapshot, CredentialPrompt, DnsObservation, LogEntry } from '../../types/ipc'

const LOG_FLUSH_INTERVAL_MS = 32
const MAX_PENDING_LOG_BATCH = 100

export function useConnectionEvents() {
  const setConnection = useAppStore((state) => state.setConnection)
  const setDnsObservation = useAppStore((state) => state.setDnsObservation)
  const appendLogs = useAppStore((state) => state.appendLogs)
  const setCredentialPrompt = useAppStore((state) => state.setCredentialPrompt)
  const pendingLogsRef = useRef<LogEntry[]>([])
  const flushTimerRef = useRef<number | null>(null)

  useEffect(() => {
    const flushLogs = () => {
      if (!pendingLogsRef.current.length) {
        return
      }

      const entries = pendingLogsRef.current
      pendingLogsRef.current = []
      appendLogs(entries)
    }

    const scheduleFlush = () => {
      if (pendingLogsRef.current.length >= MAX_PENDING_LOG_BATCH) {
        if (flushTimerRef.current !== null) {
          window.clearTimeout(flushTimerRef.current)
          flushTimerRef.current = null
        }
        flushLogs()
        return
      }

      if (flushTimerRef.current !== null) {
        return
      }

      flushTimerRef.current = window.setTimeout(() => {
        flushTimerRef.current = null
        flushLogs()
      }, LOG_FLUSH_INTERVAL_MS)
    }

    const unlisten = Promise.all([
      listen<ConnectionSnapshot>('connection://state-changed', (event) => {
        setConnection(event.payload)
      }),
      listen<LogEntry>('connection://log-line', (event) => {
        pendingLogsRef.current.push(event.payload)
        scheduleFlush()
      }),
      listen<CredentialPrompt>('connection://credentials-requested', (event) => {
        setCredentialPrompt(event.payload)
      }),
      listen<DnsObservation>('connection://dns-observed', (event) => {
        setDnsObservation(event.payload)
      }),
    ])

    return () => {
      if (flushTimerRef.current !== null) {
        window.clearTimeout(flushTimerRef.current)
        flushTimerRef.current = null
      }
      flushLogs()
      void unlisten.then((listeners) => {
        listeners.forEach((listener) => listener())
      })
    }
  }, [appendLogs, setConnection, setCredentialPrompt, setDnsObservation])
}
