import { deleteProfile as apiDeleteProfile, getProfile, getLastSelectedProfile, setLastSelectedProfile, listProfiles, updateProfileDnsPolicy, importProfile } from '../../features/profiles/api'
import { normalizeCommandError } from '../../lib/tauri'
import type { AppStoreApi } from '../createAppStore'

export function createSelectProfileAction(get: AppStoreApi['getState'], set: AppStoreApi['setState']) {
  return async (profileId: string) => {
    try {
      const selectedProfile = await getProfile(profileId)
      set({
        selectedProfileId: profileId,
        selectedProfile,
        error: null,
      })
      await setLastSelectedProfile(profileId)
    } catch (error) {
      set({ error: normalizeCommandError(error) })
    }
  }
}

export function createRefreshSelectedProfileAction(get: AppStoreApi['getState'], set: AppStoreApi['setState']) {
  return async () => {
    const selectedProfileId = get().selectedProfileId
    if (!selectedProfileId) {
      return
    }

    try {
      const selectedProfile = await getProfile(selectedProfileId)
      set({
        selectedProfile,
        error: null,
      })
    } catch (error) {
      set({ error: normalizeCommandError(error) })
    }
  }
}

export function createRefreshProfilesAction(get: AppStoreApi['getState'], set: AppStoreApi['setState']) {
  return async () => {
    try {
      const profiles = await listProfiles()
      set({ profiles, error: null })
    } catch (error) {
      set({ error: normalizeCommandError(error) })
    }
  }
}

export function createDeleteProfileAction(get: AppStoreApi['getState'], set: AppStoreApi['setState']) {
  return async (profileId: string) => {
    try {
      await apiDeleteProfile(profileId)
      await createRefreshProfilesAction(get, set)()

      const { selectedProfileId, profiles } = get()
      if (selectedProfileId === profileId) {
        const nextProfileId = profiles.length > 0 ? profiles[0].id : null
        if (nextProfileId) {
          await createSelectProfileAction(get, set)(nextProfileId)
        } else {
          set({ selectedProfileId: null, selectedProfile: null })
          await setLastSelectedProfile(null)
        }
      }
    } catch (error) {
      set({ error: normalizeCommandError(error) })
    }
  }
}

export function createUpdateSelectedProfileDnsPolicyAction(get: AppStoreApi['getState'], set: AppStoreApi['setState']) {
  return async (dnsPolicy: 'SplitDnsPreferred' | 'FullOverride' | 'ObserveOnly') => {
    const selectedProfile = get().selectedProfile
    if (!selectedProfile) {
      return
    }

    try {
      const updatedProfile = await updateProfileDnsPolicy(selectedProfile.profile.id, dnsPolicy)
      set({
        selectedProfile: updatedProfile,
        error: null,
      })
    } catch (error) {
      set({ error: normalizeCommandError(error) })
    }
  }
}

export function createBeginImportAction(get: AppStoreApi['getState'], set: AppStoreApi['setState']) {
  return async (filePath: string) => {
    try {
      const response = await importProfile(filePath, false)
      if (response.report.status !== 'Imported') {
        set({
          importWarning: {
            draft: { filePath },
            response,
          },
          error: null,
        })
        return
      }

      await createRefreshProfilesAction(get, set)()
      if (response.profile) {
        set({
          selectedProfileId: response.profile.profile.id,
          selectedProfile: response.profile,
          importWarning: null,
          error: null,
        })
        await setLastSelectedProfile(response.profile.profile.id)
      }
    } catch (error) {
      set({ error: normalizeCommandError(error) })
    }
  }
}

export function createApproveImportWarningsAction(get: AppStoreApi['getState'], set: AppStoreApi['setState']) {
  return async () => {
    const pending = get().importWarning
    if (!pending) {
      return
    }

    try {
      const response = await importProfile(pending.draft.filePath, true, pending.draft.displayName)
      if (response.report.status !== 'Imported') {
        set({
          importWarning: {
            draft: pending.draft,
            response,
          },
          error: null,
        })
        return
      }

      await createRefreshProfilesAction(get, set)()
      set({ importWarning: null, error: null })

      if (response.profile) {
        set({
          selectedProfileId: response.profile.profile.id,
          selectedProfile: response.profile,
        })
        await setLastSelectedProfile(response.profile.profile.id)
      }
    } catch (error) {
      set({ error: normalizeCommandError(error) })
    }
  }
}
