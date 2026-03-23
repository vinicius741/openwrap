import type { StateCreator } from 'zustand'
import type { UserFacingError } from '../../types/ipc'

export type ErrorSlice = {
  error: UserFacingError | null
  setError: (error: UserFacingError | null) => void
  clearError: () => void
}

export const createErrorSlice: StateCreator<ErrorSlice, [], [], ErrorSlice> = (set) => ({
  error: null,

  setError: (error) => set({ error }),

  clearError: () => set({ error: null }),
})
