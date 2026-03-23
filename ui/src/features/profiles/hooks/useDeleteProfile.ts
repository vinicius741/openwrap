import { useState, useCallback } from 'react'
import { useAppStore } from '../../../store/appStore'

interface DeleteProfileState {
  isDeleting: boolean
  error: string | null
}

interface UseDeleteProfileReturn extends DeleteProfileState {
  deleteProfile: (profileId: string) => Promise<void>
  resetError: () => void
}

export function useDeleteProfile(): UseDeleteProfileReturn {
  const deleteProfileFromStore = useAppStore((state) => state.deleteProfile)
  const [isDeleting, setIsDeleting] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const resetError = useCallback(() => {
    setError(null)
  }, [])

  const deleteProfile = useCallback(
    async (profileId: string) => {
      setIsDeleting(true)
      setError(null)
      try {
        await deleteProfileFromStore(profileId)
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Failed to delete profile')
        throw err
      } finally {
        setIsDeleting(false)
      }
    },
    [deleteProfileFromStore],
  )

  return { isDeleting, error, deleteProfile, resetError }
}
