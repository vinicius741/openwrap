import type { StateCreator } from 'zustand'
import type { ImportWarningState } from '../../types/domain'

export type ImportSlice = {
  importWarning: ImportWarningState | null
  setImportWarning: (warning: ImportWarningState | null) => void
  clearImportWarning: () => void
}

export const createImportSlice: StateCreator<ImportSlice, [], [], ImportSlice> = (set) => ({
  importWarning: null,

  setImportWarning: (importWarning) => set({ importWarning }),

  clearImportWarning: () => set({ importWarning: null }),
})
