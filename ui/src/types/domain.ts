export interface ImportDraft {
  filePath: string
  displayName?: string
}

export interface ImportWarningState {
  draft: ImportDraft
  response: import('./ipc').ImportProfileResponse
}

