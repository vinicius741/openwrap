import type { StateCreator } from 'zustand'
import type { OpenVpnDetection, Settings } from '../../types/ipc'

export type SettingsSlice = {
  settings: Settings | null
  detection: OpenVpnDetection | null
  setSettings: (settings: Settings) => void
  setDetection: (detection: OpenVpnDetection) => void
}

export const createSettingsSlice: StateCreator<SettingsSlice, [], [], SettingsSlice> = (set) => ({
  settings: null,
  detection: null,

  setSettings: (settings) => set({ settings }),

  setDetection: (detection) => set({ detection }),
})
