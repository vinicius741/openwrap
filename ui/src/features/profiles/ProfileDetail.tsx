import { useState, useCallback } from 'react'
import { EmptyState } from '../../components/EmptyState'
import { ConnectionPanel } from '../connection/ConnectionPanel'
import { LogPane } from '../logs/LogPane'
import { ProfileHeader } from './components/ProfileHeader'
import { DeleteProfileDialog } from './components/DeleteProfileDialog'
import { ConfigSection } from './components/ConfigSection'
import { ProfileInfo } from './components/ProfileInfo'
import { useDeleteProfile } from './hooks/useDeleteProfile'
import { useAppStore } from '../../store/appStore'
import type { DnsPolicy } from '../../types/ipc'

export function ProfileDetail() {
  const profile = useAppStore((state) => state.selectedProfile)
  const updateSelectedProfileDnsPolicy = useAppStore((state) => state.updateSelectedProfileDnsPolicy)
  const updateSelectedProfileDnsMatchDomains = useAppStore(
    (state) => state.updateSelectedProfileDnsMatchDomains,
  )
  const reconnectSelected = useAppStore((state) => state.reconnectSelected)
  const dnsChangePendingProfileId = useAppStore((state) => state.dnsChangePendingProfileId)
  const configureSelectedProfileGeneratedPassword = useAppStore(
    (state) => state.configureSelectedProfileGeneratedPassword,
  )
  const clearSelectedProfileGeneratedPassword = useAppStore(
    (state) => state.clearSelectedProfileGeneratedPassword,
  )
  const [showDeleteModal, setShowDeleteModal] = useState(false)
  const [isUpdatingPolicy, setIsUpdatingPolicy] = useState(false)
  const [isReconnecting, setIsReconnecting] = useState(false)
  const [isSavingGeneratedPassword, setIsSavingGeneratedPassword] = useState(false)
  const [isClearingGeneratedPassword, setIsClearingGeneratedPassword] = useState(false)
  const { isDeleting, error: deleteError, deleteProfile, resetError } = useDeleteProfile()

  const closeModal = useCallback(() => {
    setShowDeleteModal(false)
    resetError()
  }, [resetError])

  const handleDelete = useCallback(async () => {
    if (!profile) return
    try {
      await deleteProfile(profile.profile.id)
      closeModal()
    } catch {
      // Error is handled by the hook
    }
  }, [profile, deleteProfile, closeModal])

  const handleDnsPolicyChange = useCallback(
    async (dnsPolicy: DnsPolicy) => {
      setIsUpdatingPolicy(true)
      try {
        await updateSelectedProfileDnsPolicy(dnsPolicy)
      } finally {
        setIsUpdatingPolicy(false)
      }
    },
    [updateSelectedProfileDnsPolicy],
  )

  const handleDnsDomainsChange = useCallback(
    async (domains: string[]) => {
      setIsUpdatingPolicy(true)
      try {
        await updateSelectedProfileDnsMatchDomains(domains)
      } finally {
        setIsUpdatingPolicy(false)
      }
    },
    [updateSelectedProfileDnsMatchDomains],
  )

  const handleReconnect = useCallback(async () => {
    setIsReconnecting(true)
    try {
      await reconnectSelected()
    } finally {
      setIsReconnecting(false)
    }
  }, [reconnectSelected])

  const handleGeneratedPasswordSave = useCallback(
    async (input: { username: string; pin: string; totpSecret: string }) => {
      setIsSavingGeneratedPassword(true)
      try {
        await configureSelectedProfileGeneratedPassword(input)
      } finally {
        setIsSavingGeneratedPassword(false)
      }
    },
    [configureSelectedProfileGeneratedPassword],
  )

  const handleGeneratedPasswordClear = useCallback(async () => {
    setIsClearingGeneratedPassword(true)
    try {
      await clearSelectedProfileGeneratedPassword()
    } finally {
      setIsClearingGeneratedPassword(false)
    }
  }, [clearSelectedProfileGeneratedPassword])

  if (!profile) {
    return <EmptyState title="No profile selected" detail="Import a profile and select it from the sidebar." />
  }

  return (
    <div className="profile-detail">
      <ProfileHeader
        name={profile.profile.name}
        remoteSummary={profile.profile.remote_summary}
        onDeleteClick={() => setShowDeleteModal(true)}
      />

      <DeleteProfileDialog
        isOpen={showDeleteModal}
        profileName={profile.profile.name}
        isDeleting={isDeleting}
        error={deleteError}
        onClose={closeModal}
        onConfirm={handleDelete}
      />

      <ConnectionPanel />

      <ConfigSection
        profile={profile}
        isSavingPassword={isSavingGeneratedPassword}
        isClearingPassword={isClearingGeneratedPassword}
        isUpdatingDns={isUpdatingPolicy}
        dnsNeedsReconnect={dnsChangePendingProfileId === profile.profile.id}
        isReconnecting={isReconnecting}
        onDnsPolicyChange={handleDnsPolicyChange}
        onDnsDomainsChange={handleDnsDomainsChange}
        onReconnect={handleReconnect}
        onGeneratedPasswordSave={handleGeneratedPasswordSave}
        onGeneratedPasswordClear={handleGeneratedPasswordClear}
      />

      <LogPane />

      <ProfileInfo findings={profile.findings} assets={profile.assets} />
    </div>
  )
}
