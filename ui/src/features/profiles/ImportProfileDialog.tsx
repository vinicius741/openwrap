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
            <h3>{importWarning.response.report.status === 'Blocked' ? 'Import blocked' : 'Approve risky directives'}</h3>
            <p>
              {importWarning.response.report.status === 'Blocked'
                ? 'OpenWrap blocked this profile because it does not fit the current import policy.'
                : 'This profile uses directives that change routing or environment behavior. Review the warnings before importing.'}
            </p>
            {importWarning.response.report.warnings.length ? (
              <ul className="finding-list">
                {importWarning.response.report.warnings.map((finding) => (
                  <li key={`${finding.directive}-${finding.line}`}>
                    <strong>{finding.directive}</strong> on line {finding.line}: {finding.message}
                  </li>
                ))}
              </ul>
            ) : null}
            {importWarning.response.report.errors.length ? (
              <ul className="finding-list">
                {importWarning.response.report.errors.map((error) => (
                  <li key={error}>{error}</li>
                ))}
              </ul>
            ) : null}
            <div className="modal-actions">
              <button className="action-button action-secondary" onClick={() => clearImportWarning()} type="button">
                Close
              </button>
              {importWarning.response.report.status === 'NeedsApproval' ? (
                <button className="action-button action-primary" onClick={() => void approveImportWarnings()} type="button">
                  Import anyway
                </button>
              ) : null}
            </div>
          </div>
        </div>
      ) : null}
    </>
  )
}
