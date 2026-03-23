import { normalizeCommandError } from '../../lib/tauri'
import { revealConnectionLogInFinder } from '../logs/api'
import { useAppStore } from '../../store/appStore'
import { isConnected, getDnsStatusMessage } from './model/status'
import { ConnectionSummary } from './components/ConnectionSummary'
import { ConnectionControls } from './components/ConnectionControls'
import { ConnectionMetadata } from './components/ConnectionMetadata'
import { DnsStatusNotice } from './components/DnsStatusNotice'
import { ConnectionErrorBanner } from './components/ConnectionErrorBanner'
import { CredentialPromptForm } from './components/CredentialPromptForm'

export function ConnectionPanel() {
  const connection = useAppStore((state) => state.connection)
  const selectedProfile = useAppStore((state) => state.selectedProfile)
  const prompt = useAppStore((state) => state.pendingCredentialPrompt)
  const connectSelected = useAppStore((state) => state.connectSelected)
  const disconnect = useAppStore((state) => state.disconnect)
  const submit = useAppStore((state) => state.submitCredentials)
  const setError = useAppStore((state) => state.setError)

  const dnsStatusMessage = getDnsStatusMessage(connection?.dns_observation)

  const handleShowLogs = () => {
    document.getElementById('connection-logs')?.scrollIntoView({
      behavior: 'smooth',
      block: 'start',
    })
  }

  const handleRevealLog = async () => {
    try {
      await revealConnectionLogInFinder()
    } catch (error) {
      setError(normalizeCommandError(error))
    }
  }

  return (
    <section className="connection-panel">
      <ConnectionSummary connection={connection ?? undefined} selectedProfile={selectedProfile ?? undefined} />

      <ConnectionControls
        connection={connection ?? undefined}
        onConnect={() => void connectSelected()}
        onDisconnect={() => void disconnect()}
        metadata={<ConnectionMetadata connection={connection ?? undefined} selectedProfile={selectedProfile ?? undefined} />}
      />

      <DnsStatusNotice message={dnsStatusMessage} />

      <ConnectionErrorBanner
        error={connection?.last_error ?? null}
        logFilePath={connection?.log_file_path}
        onShowLogs={handleShowLogs}
        onRevealLog={handleRevealLog}
      />

      <CredentialPromptForm
        prompt={prompt ?? null}
        onSubmit={(credentials) =>
          void submit(credentials)
        }
      />
    </section>
  )
}
