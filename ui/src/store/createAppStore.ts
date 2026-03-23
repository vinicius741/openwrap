import type { StoreApi, UseBoundStore } from 'zustand'

/**
 * Type alias for the store API, useful for action creators.
 */
export type AppStoreApi = StoreApi<AppStoreState>

/**
 * Type alias for the useAppStore hook.
 */
export type AppStoreHook = UseBoundStore<AppStoreApi>

import type {
  ProfileSlice,
  ConnectionSlice,
  SettingsSlice,
  ImportSlice,
  ErrorSlice,
} from './slices'

/**
 * The combined state of all slices.
 */
export type AppStoreState = ProfileSlice & ConnectionSlice & SettingsSlice & ImportSlice & ErrorSlice

/**
 * Re-export UiLogEntry from connection slice for backward compatibility.
 */
export type { UiLogEntry } from './slices'
