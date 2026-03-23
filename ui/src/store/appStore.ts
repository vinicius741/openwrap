import { createAppStore } from './createAppStore'

export const useAppStore = createAppStore()

export type UiLogEntry = import('./slices/connectionSlice').UiLogEntry
export type AppStore = ReturnType<typeof createAppStore>
