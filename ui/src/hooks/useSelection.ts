import { useAppStore } from '../store/appStore'

export function useSelection() {
  return useAppStore((state) => ({
    selectedProfileId: state.selectedProfileId,
    selectedProfile: state.selectedProfile,
  }))
}
