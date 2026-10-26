import type { AppInfo, UpdateInfo, UpdateProgress, UserFacingError } from '../../types/ipc'

export type UpdateStatus = 'idle' | 'checking' | 'up_to_date' | 'available' | 'installing' | 'error'

export type UpdateSlice = {
  appInfo: AppInfo | null
  availableUpdate: UpdateInfo | null
  updateStatus: UpdateStatus
  updateProgress: UpdateProgress | null
  updateError: UserFacingError | null
  updateBannerDismissed: boolean
}

export const updateInitialState: UpdateSlice = {
  appInfo: null,
  availableUpdate: null,
  updateStatus: 'idle',
  updateProgress: null,
  updateError: null,
  updateBannerDismissed: false,
}
