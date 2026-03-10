import { open } from '@tauri-apps/plugin-dialog'

import { useAppStore } from '../../store/appStore'

export function ImportProfileDialog() {
  const beginImport = useAppStore((state) => state.beginImport)
  const approveImportWarnings = useAppStore((state) => state.approveImportWarnings)
  const clearImportWarning = useAppStore((state) => state.clearImportWarning)
  const importWarning = useAppStore((state) => state.importWarning)

  async function handleImport() {
    const filePath = await open({
      filters: [{ name: 'OpenVPN Profiles', extensions: ['ovpn'] }],
      multiple: false,
    })

    if (typeof filePath === 'string') {
      await beginImport(filePath)
    }
  }

  return (
    <>
      <button className="action-button action-primary" onClick={() => void handleImport()} type="button">
        Import
      </button>
      {importWarning ? (
        <div className="modal-backdrop">
          <div className="modal">
            <h3>Approve risky directives</h3>
            <p>
              This profile uses directives that change routing or environment behavior. Review the warnings before
              importing.
            </p>
            <ul className="finding-list">
              {importWarning.response.report.warnings.map((finding) => (
                <li key={`${finding.directive}-${finding.line}`}>
                  <strong>{finding.directive}</strong> on line {finding.line}: {finding.message}
                </li>
              ))}
            </ul>
            <div className="modal-actions">
              <button className="action-button action-secondary" onClick={() => clearImportWarning()} type="button">
                Cancel
              </button>
              <button className="action-button action-primary" onClick={() => void approveImportWarnings()} type="button">
                Import anyway
              </button>
            </div>
          </div>
        </div>
      ) : null}
    </>
  )
}
