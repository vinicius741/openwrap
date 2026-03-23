import type { ImportWarningState } from '../../types/domain'

export type ImportSlice = {
  importWarning: ImportWarningState | null
}

export const importInitialState: ImportSlice = {
  importWarning: null,
}
