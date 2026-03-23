#[cfg(test)]
mod tests {
    use std::collections::{HashMap, VecDeque};
    use std::fs;
    use std::sync::Arc;
    use std::time::Duration;

    use chrono::Utc;
    use parking_lot::Mutex;
    use tempfile::tempdir;
    use tokio::sync::mpsc;

    use crate::connection::manager::state::ConnectionManager;
    use crate::connection::manager::CoreEvent;
    use crate::app_state::AppPaths;
    use crate::connection::{ConnectionState, CredentialSubmission, SessionId};
    use crate::dns::DnsPolicy;
    use crate::errors::AppError;
    use crate::openvpn::{BackendEvent, ConnectRequest, ReconcileDnsRequest, SpawnedSession};
    use crate::profiles::repository::ProfileRepository;
    use crate::profiles::{
        AssetId, AssetKind, AssetOrigin, CredentialMode, ManagedAsset, Profile, ProfileDetail,
        ProfileId, ProfileImportResult, ProfileSummary, ValidationFinding, ValidationStatus,
    };
    use crate::secrets::StoredSecret;
    use crate::{SecretStore, VpnBackend};

    #[derive(Default)]
    struct FakeSecretStore {
        secrets: Mutex<HashMap<ProfileId, StoredSecret>>,
    }

    impl SecretStore for FakeSecretStore {
        fn get_password(&self, profile_id: &ProfileId) -> Result<Option<StoredSecret>, AppError> {
            Ok(self.secrets.lock().get(profile_id).cloned())
        }

        fn set_password(&self, secret: StoredSecret) -> Result<(), AppError> {
            self.secrets
                .lock()
                .insert(secret.profile_id.clone(), secret);
            Ok(())
        }

        fn delete_password(&self, profile_id: &ProfileId) -> Result<(), AppError> {
            self.secrets.lock().remove(profile_id);
            Ok(())
        }
    }

    struct FakeRepository {
        detail: ProfileDetail,
        settings: crate::openvpn::runtime::Settings,
        last_selected: Mutex<Option<ProfileId>>,
        touch_count: Mutex<u32>,
        saved_credentials: Mutex<bool>,
        dns_policy_updates: Mutex<Vec<DnsPolicy>>,
        dns_policy_update_error: Mutex<Option<String>>,
    }

    impl ProfileRepository for FakeRepository {
        fn save_import(&self, _import: ProfileImportResult) -> Result<ProfileDetail, AppError> {
            unreachable!()
        }

        fn list_profiles(&self) -> Result<Vec<ProfileSummary>, AppError> {
            Ok(vec![])
        }

        fn get_profile(&self, profile_id: &ProfileId) -> Result<ProfileDetail, AppError> {
            if &self.detail.profile.id == profile_id {
                Ok(self.detail.clone())
            } else {
                Err(AppError::ProfileNotFound(profile_id.to_string()))
            }
        }

        fn update_has_saved_credentials(
            &self,
            _profile_id: &ProfileId,
            has_saved_credentials: bool,
        ) -> Result<(), AppError> {
            *self.saved_credentials.lock() = has_saved_credentials;
            Ok(())
        }

        fn touch_last_used(&self, _profile_id: &ProfileId) -> Result<(), AppError> {
            *self.touch_count.lock() += 1;
            Ok(())
        }

        fn get_settings(&self) -> Result<crate::openvpn::runtime::Settings, AppError> {
            Ok(self.settings.clone())
        }

        fn save_settings(
            &self,
            _settings: &crate::openvpn::runtime::Settings,
        ) -> Result<(), AppError> {
            unreachable!()
        }

        fn list_validation_findings(
            &self,
            _profile_id: &ProfileId,
        ) -> Result<Vec<ValidationFinding>, AppError> {
            Ok(vec![])
        }

        fn update_profile_dns_policy(
            &self,
            _profile_id: &ProfileId,
            policy: DnsPolicy,
        ) -> Result<ProfileDetail, AppError> {
            if let Some(error) = self.dns_policy_update_error.lock().as_ref() {
                return Err(AppError::ConnectionState(error.clone()));
            }

            self.dns_policy_updates.lock().push(policy.clone());
            let mut detail = self.detail.clone();
            detail.profile.dns_policy = policy;
            Ok(detail)
        }

        fn set_last_selected_profile(
            &self,
            profile_id: Option<&ProfileId>,
        ) -> Result<(), AppError> {
            *self.last_selected.lock() = profile_id.cloned();
            Ok(())
        }

        fn get_last_selected_profile(&self) -> Result<Option<ProfileId>, AppError> {
            Ok(self.last_selected.lock().clone())
        }

        fn delete_profile(&self, _profile_id: &ProfileId) -> Result<(), AppError> {
            Ok(())
        }
    }

    enum QueuedConnect {
        Session {
            pid: Option<u32>,
            event_rx: mpsc::UnboundedReceiver<BackendEvent>,
        },
        Error(AppError),
    }

    #[derive(Default)]
    struct FakeBackendState {
        queue: VecDeque<QueuedConnect>,
        requests: Vec<ConnectRequest>,
        disconnects: Vec<SessionId>,
        reconcile_requests: Vec<ReconcileDnsRequest>,
        reconcile_results: VecDeque<Result<(), AppError>>,
    }

    #[derive(Clone, Default)]
    struct FakeBackend {
        state: Arc<Mutex<FakeBackendState>>,
    }

    #[derive(Clone)]
    struct ScriptedSession {
        tx: mpsc::UnboundedSender<BackendEvent>,
    }

    impl FakeBackend {
        fn queue_session(&self, pid: Option<u32>) -> ScriptedSession {
            let (tx, rx) = mpsc::unbounded_channel();
            self.state
                .lock()
                .queue
                .push_back(QueuedConnect::Session { pid, event_rx: rx });
            ScriptedSession { tx }
        }

        fn queue_error(&self, error: AppError) {
            self.state
                .lock()
                .queue
                .push_back(QueuedConnect::Error(error));
        }

        fn request_count(&self) -> usize {
            self.state.lock().requests.len()
        }

        fn last_request(&self) -> Option<ConnectRequest> {
            self.state.lock().requests.last().cloned()
        }

        fn disconnect_count(&self) -> usize {
            self.state.lock().disconnects.len()
        }

        fn queue_reconcile_result(&self, result: Result<(), AppError>) {
            self.state.lock().reconcile_results.push_back(result);
        }

        fn reconcile_count(&self) -> usize {
            self.state.lock().reconcile_requests.len()
        }
    }

    impl VpnBackend for FakeBackend {
        fn connect(&self, request: ConnectRequest) -> Result<SpawnedSession, AppError> {
            self.state.lock().requests.push(request.clone());
            match self
                .state
                .lock()
                .queue
                .pop_front()
                .expect("expected queued connection")
            {
                QueuedConnect::Session { pid, event_rx } => Ok(SpawnedSession {
                    session_id: request.session_id,
                    pid,
                    event_rx,
                }),
                QueuedConnect::Error(error) => Err(error),
            }
        }

        fn disconnect(&self, session_id: SessionId) -> Result<(), AppError> {
            self.state.lock().disconnects.push(session_id);
            Ok(())
        }

        fn reconcile_dns(&self, request: ReconcileDnsRequest) -> Result<(), AppError> {
            let mut state = self.state.lock();
            state.reconcile_requests.push(request);
            state.reconcile_results.pop_front().unwrap_or(Ok(()))
        }
    }

    fn build_manager(
        credential_mode: CredentialMode,
        saved_username: Option<&str>,
    ) -> (
        ConnectionManager,
        FakeBackend,
        ProfileId,
        Arc<FakeSecretStore>,
        Arc<FakeRepository>,
    ) {
        let temp = tempdir().unwrap();
        let base_dir = temp.path().to_path_buf();
        std::mem::forget(temp);

        let paths = AppPaths::new(&base_dir);
        paths.ensure().unwrap();

        let openvpn_path = base_dir.join("openvpn");
        fs::write(&openvpn_path, "#!/bin/sh\n").unwrap();

        let profile_id = ProfileId::new();
        let managed_dir = base_dir.join("profiles").join(profile_id.to_string());
        fs::create_dir_all(&managed_dir).unwrap();
        let asset_path = managed_dir.join("assets").join("tls-auth.key");
        fs::create_dir_all(asset_path.parent().unwrap()).unwrap();
        fs::write(&asset_path, "static-key").unwrap();
        let managed_ovpn_path = managed_dir.join("config.ovpn");
        fs::write(
            &managed_ovpn_path,
            "client\nremote example.com 1194\ntls-auth assets/tls-auth.key 1\n",
        )
        .unwrap();

        let detail = ProfileDetail {
            profile: Profile {
                id: profile_id.clone(),
                name: "Test".into(),
                source_filename: "test.ovpn".into(),
                managed_dir,
                managed_ovpn_path,
                original_import_path: base_dir.join("test.ovpn"),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                dns_intent: vec!["DNS 1.1.1.1".into()],
                dns_policy: DnsPolicy::SplitDnsPreferred,
                credential_mode,
                remote_summary: "example.com:1194".into(),
                has_saved_credentials: false,
                validation_status: ValidationStatus::Ok,
            },
            assets: vec![ManagedAsset {
                id: AssetId::new(),
                profile_id: profile_id.clone(),
                kind: AssetKind::TlsAuth,
                relative_path: "assets/tls-auth.key".into(),
                sha256: "sha".into(),
                origin: AssetOrigin::CopiedFile,
            }],
            findings: vec![],
        };
        let repository = Arc::new(FakeRepository {
            detail,
            settings: crate::openvpn::runtime::Settings {
                openvpn_path_override: Some(openvpn_path),
                verbose_logging: false,
            },
            last_selected: Mutex::new(None),
            touch_count: Mutex::new(0),
            saved_credentials: Mutex::new(saved_username.is_some()),
            dns_policy_updates: Mutex::new(Vec::new()),
            dns_policy_update_error: Mutex::new(None),
        });
        let backend = FakeBackend::default();
        let secret_store = Arc::new(FakeSecretStore::default());
        if let Some(username) = saved_username {
            secret_store
                .set_password(StoredSecret {
                    profile_id: profile_id.clone(),
                    username: username.into(),
                })
                .unwrap();
        }
        let manager = ConnectionManager::new(
            paths,
            repository.clone(),
            secret_store.clone(),
            Arc::new(backend.clone()),
        );

        (manager, backend, profile_id, secret_store, repository)
    }

    #[tokio::test(start_paused = true)]
    async fn retries_after_exit_and_recovers() {
        let (manager, backend, profile_id, _, _) = build_manager(CredentialMode::None, None);
        let first = backend.queue_session(Some(41));
        let second = backend.queue_session(Some(42));

        manager.connect(profile_id.to_string()).await.unwrap();
        assert_eq!(manager.snapshot().state, ConnectionState::Connecting);

        let first_runtime = backend.last_request().unwrap().runtime_dir;
        assert!(first_runtime.exists());

        first.tx.send(BackendEvent::Exited(Some(1))).unwrap();
        tokio::task::yield_now().await;

        let reconnecting = manager.snapshot();
        assert_eq!(reconnecting.state, ConnectionState::Reconnecting);
        assert_eq!(reconnecting.retry_count, 1);
        assert_eq!(
            reconnecting.substate.as_deref(),
            Some("Retrying in 2 seconds")
        );
        assert!(!first_runtime.exists());

        tokio::time::advance(Duration::from_secs(2)).await;
        tokio::task::yield_now().await;

        assert_eq!(backend.request_count(), 2);
        assert_eq!(manager.snapshot().state, ConnectionState::Connecting);

        second
            .tx
            .send(BackendEvent::Stdout(
                "Initialization Sequence Completed".into(),
            ))
            .unwrap();
        tokio::task::yield_now().await;

        let connected = manager.snapshot();
        assert_eq!(connected.state, ConnectionState::Connected);
        assert_eq!(connected.retry_count, 1);
    }

    #[tokio::test]
    async fn writes_runtime_launch_config_with_absolute_asset_paths() {
        let (manager, backend, profile_id, _, repository) =
            build_manager(CredentialMode::None, None);
        let session = backend.queue_session(Some(43));
        let asset_path = repository
            .detail
            .profile
            .managed_dir
            .join("assets")
            .join("tls-auth.key");

        manager.connect(profile_id.to_string()).await.unwrap();

        let request = backend.last_request().unwrap();
        let launch_config = fs::read_to_string(&request.config_path).unwrap();
        assert!(request.config_path.starts_with(&request.runtime_dir));
        assert!(launch_config.contains(&format!(
            "tls-auth {} 1",
            asset_path.display().to_string().replace(' ', "\\ ")
        )));

        session.tx.send(BackendEvent::Exited(Some(0))).unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn stops_after_retry_budget_is_exhausted() {
        let (manager, backend, profile_id, _, _) = build_manager(CredentialMode::None, None);
        let sessions = [
            backend.queue_session(Some(11)),
            backend.queue_session(Some(12)),
            backend.queue_session(Some(13)),
            backend.queue_session(Some(14)),
        ];

        manager.connect(profile_id.to_string()).await.unwrap();
        let delays = [2_u64, 5, 10];

        for (index, delay) in delays.into_iter().enumerate() {
            sessions[index]
                .tx
                .send(BackendEvent::Exited(Some(1)))
                .unwrap();
            tokio::task::yield_now().await;
            assert_eq!(manager.snapshot().state, ConnectionState::Reconnecting);
            tokio::time::advance(Duration::from_secs(delay)).await;
            tokio::task::yield_now().await;
        }

        sessions[3].tx.send(BackendEvent::Exited(Some(1))).unwrap();
        tokio::task::yield_now().await;

        let failed = manager.snapshot();
        assert_eq!(backend.request_count(), 4);
        assert_eq!(failed.state, ConnectionState::Error);
        assert_eq!(failed.retry_count, 3);
        assert_eq!(
            failed.last_error.as_ref().map(|error| error.code.as_str()),
            Some("process_exit")
        );
    }

    #[tokio::test(start_paused = true)]
    async fn surfaces_last_openvpn_diagnostic_after_retry_budget_is_exhausted() {
        let (manager, backend, profile_id, _, _) = build_manager(CredentialMode::None, None);
        let sessions = [
            backend.queue_session(Some(21)),
            backend.queue_session(Some(22)),
            backend.queue_session(Some(23)),
            backend.queue_session(Some(24)),
        ];

        manager.connect(profile_id.to_string()).await.unwrap();
        let delays = [2_u64, 5, 10];

        for (index, delay) in delays.into_iter().enumerate() {
            sessions[index]
                .tx
                .send(BackendEvent::Exited(Some(1)))
                .unwrap();
            tokio::task::yield_now().await;
            tokio::time::advance(Duration::from_secs(delay)).await;
            tokio::task::yield_now().await;
        }

        sessions[3]
            .tx
            .send(BackendEvent::Stderr(
                "RESOLVE: Cannot resolve host address: vpn.example.invalid:1194".into(),
            ))
            .unwrap();
        tokio::task::yield_now().await;
        sessions[3].tx.send(BackendEvent::Exited(Some(1))).unwrap();
        tokio::task::yield_now().await;

        let failed = manager.snapshot();
        let last_error = failed.last_error.expect("expected terminal error");
        assert_eq!(failed.state, ConnectionState::Error);
        assert_eq!(last_error.code, "openvpn_host_resolution_failed");
        assert!(last_error
            .details_safe
            .as_deref()
            .is_some_and(|detail| detail.contains("Cannot resolve host address")));
    }

    #[tokio::test(start_paused = true)]
    async fn terminal_failures_persist_a_sanitized_log_file() {
        let (manager, backend, profile_id, _, _) = build_manager(CredentialMode::None, None);
        let sessions = [
            backend.queue_session(Some(25)),
            backend.queue_session(Some(26)),
            backend.queue_session(Some(27)),
            backend.queue_session(Some(28)),
        ];

        manager.connect(profile_id.to_string()).await.unwrap();
        let delays = [2_u64, 5, 10];

        for (index, delay) in delays.into_iter().enumerate() {
            sessions[index]
                .tx
                .send(BackendEvent::Exited(Some(78)))
                .unwrap();
            tokio::task::yield_now().await;
            tokio::time::advance(Duration::from_secs(delay)).await;
            tokio::task::yield_now().await;
        }

        sessions[3]
            .tx
            .send(BackendEvent::Stderr(
                "Options error: PASSWORD verification failed".into(),
            ))
            .unwrap();
        tokio::task::yield_now().await;
        sessions[3].tx.send(BackendEvent::Exited(Some(78))).unwrap();
        tokio::task::yield_now().await;

        let failed = manager.snapshot();
        let log_path = manager.paths.failed_connection_log_path();
        let saved_path = failed
            .log_file_path
            .as_deref()
            .expect("expected persisted log path");
        let saved_log = fs::read_to_string(&log_path).unwrap();

        assert_eq!(saved_path, log_path.to_string_lossy());
        assert!(saved_log.contains("[redacted] verification failed"));
        assert!(!saved_log.contains("PASSWORD verification failed"));
        assert_eq!(
            failed.last_error.as_ref().map(|error| error.code.as_str()),
            Some("openvpn_options_error")
        );
    }

    #[tokio::test]
    async fn cleans_runtime_artifacts_for_credentials_and_disconnect() {
        let (manager, backend, profile_id, _, _) = build_manager(CredentialMode::UserPass, None);
        let session = backend.queue_session(Some(51));

        manager.connect(profile_id.to_string()).await.unwrap();
        assert_eq!(
            manager.snapshot().state,
            ConnectionState::AwaitingCredentials
        );

        manager
            .submit_credentials(CredentialSubmission {
                profile_id: profile_id.clone(),
                username: "alice".into(),
                password: "secret".into(),
                remember_in_keychain: false,
            })
            .await
            .unwrap();

        let request = backend.last_request().unwrap();
        let auth_file = request.auth_file.clone().unwrap();
        let runtime_dir = request.runtime_dir.clone();
        assert!(auth_file.exists());
        assert!(runtime_dir.exists());

        manager.disconnect().await.unwrap();
        assert_eq!(backend.disconnect_count(), 1);
        assert!(!auth_file.exists());
        assert!(runtime_dir.exists());

        session.tx.send(BackendEvent::Exited(Some(0))).unwrap();
        tokio::task::yield_now().await;

        assert!(!auth_file.exists());
        assert!(!runtime_dir.exists());
        assert_eq!(manager.snapshot().state, ConnectionState::Idle);
        assert_eq!(backend.reconcile_count(), 1);
    }

    #[tokio::test]
    async fn auto_promotion_persists_full_override_once_per_connection() {
        let (manager, backend, profile_id, _, repository) =
            build_manager(CredentialMode::None, None);
        let session = backend.queue_session(Some(53));

        manager.connect(profile_id.to_string()).await.unwrap();

        session
            .tx
            .send(BackendEvent::Stdout(
                "OPENWRAP_DNS_WARNING: AUTO_PROMOTED_FULL_OVERRIDE".into(),
            ))
            .unwrap();
        session
            .tx
            .send(BackendEvent::Stdout(
                "OPENWRAP_DNS_WARNING: AUTO_PROMOTED_FULL_OVERRIDE".into(),
            ))
            .unwrap();
        tokio::task::yield_now().await;

        assert_eq!(
            repository.dns_policy_updates.lock().as_slice(),
            &[DnsPolicy::FullOverride]
        );
        assert_eq!(
            manager.snapshot().dns_observation.auto_promoted_policy,
            Some(DnsPolicy::FullOverride)
        );

        session.tx.send(BackendEvent::Exited(Some(0))).unwrap();
    }

    #[tokio::test]
    async fn disconnect_reconcile_failure_surfaces_dns_restore_error() {
        let (manager, backend, profile_id, _, _) = build_manager(CredentialMode::None, None);
        let session = backend.queue_session(Some(54));
        backend.queue_reconcile_result(Err(AppError::ConnectionState(
            "restore verification failed".into(),
        )));

        manager.connect(profile_id.to_string()).await.unwrap();
        manager.disconnect().await.unwrap();

        session.tx.send(BackendEvent::Exited(Some(0))).unwrap();
        tokio::task::yield_now().await;

        let snapshot = manager.snapshot();
        assert_eq!(snapshot.state, ConnectionState::Error);
        assert_eq!(
            snapshot
                .last_error
                .as_ref()
                .map(|error| error.code.as_str()),
            Some("dns_restore_failed")
        );
        assert_eq!(
            snapshot.dns_observation.restore_status,
            Some(crate::dns::DnsRestoreStatus::PendingReconcile)
        );
        assert_eq!(backend.reconcile_count(), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn auth_failures_do_not_retry() {
        let (manager, backend, profile_id, _, _) = build_manager(CredentialMode::None, None);
        let session = backend.queue_session(Some(61));

        manager.connect(profile_id.to_string()).await.unwrap();
        session
            .tx
            .send(BackendEvent::Stdout("AUTH_FAILED".into()))
            .unwrap();
        tokio::task::yield_now().await;
        session.tx.send(BackendEvent::Exited(Some(1))).unwrap();
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(20)).await;
        tokio::task::yield_now().await;

        let failed = manager.snapshot();
        assert_eq!(failed.state, ConnectionState::Error);
        assert_eq!(backend.request_count(), 1);
        assert_eq!(backend.disconnect_count(), 1);
        assert_eq!(
            failed.last_error.as_ref().map(|error| error.code.as_str()),
            Some("auth_failed")
        );
    }

    #[tokio::test(start_paused = true)]
    async fn auth_failures_persist_the_latest_failed_log() {
        let (manager, backend, profile_id, _, _) = build_manager(CredentialMode::None, None);
        let session = backend.queue_session(Some(62));

        manager.connect(profile_id.to_string()).await.unwrap();
        session
            .tx
            .send(BackendEvent::Stderr("AUTH_FAILED: bad credentials".into()))
            .unwrap();
        tokio::task::yield_now().await;
        session.tx.send(BackendEvent::Exited(Some(1))).unwrap();
        tokio::task::yield_now().await;

        let failed = manager.snapshot();
        let log_path = manager.paths.failed_connection_log_path();
        let expected_path = log_path.to_string_lossy().into_owned();
        assert_eq!(
            failed.log_file_path.as_deref(),
            Some(expected_path.as_str())
        );
        assert!(fs::read_to_string(log_path)
            .unwrap()
            .contains("AUTH_FAILED: bad credentials"));
    }

    #[tokio::test]
    async fn launch_failures_surface_as_terminal_errors() {
        let (manager, backend, profile_id, _, _) = build_manager(CredentialMode::None, None);
        backend.queue_error(AppError::OpenVpnLaunch("permission denied".into()));

        let error = manager.connect(profile_id.to_string()).await.unwrap_err();
        assert!(matches!(error, AppError::OpenVpnLaunch(_)));

        let failed = manager.snapshot();
        assert_eq!(failed.state, ConnectionState::Error);
        assert_eq!(
            failed.last_error.as_ref().map(|error| error.code.as_str()),
            Some("openvpn_launch_failed")
        );
        assert_eq!(failed.log_file_path, None);
        assert!(failed
            .last_error
            .as_ref()
            .and_then(|error| error.suggested_fix.as_deref())
            .is_some_and(|fix| !fix.contains("Show logs")));
    }

    #[tokio::test]
    async fn plain_log_lines_do_not_emit_state_changed_events() {
        let (manager, backend, profile_id, _, _) = build_manager(CredentialMode::None, None);
        let session = backend.queue_session(Some(72));
        let mut events = manager.subscribe();

        manager.connect(profile_id.to_string()).await.unwrap();
        tokio::task::yield_now().await;

        while events.try_recv().is_ok() {}

        session
            .tx
            .send(BackendEvent::Stdout("NOTE: still negotiating".into()))
            .unwrap();
        tokio::task::yield_now().await;

        let mut state_changed = 0;
        let mut log_lines = 0;

        loop {
            match events.try_recv() {
                Ok(CoreEvent::StateChanged(_)) => state_changed += 1,
                Ok(CoreEvent::LogLine(_)) => log_lines += 1,
                Ok(CoreEvent::CredentialsRequested(_) | CoreEvent::DnsObserved(_)) => {}
                Err(tokio::sync::broadcast::error::TryRecvError::Empty) => break,
                Err(error) => panic!("unexpected event receive error: {error}"),
            }
        }

        assert_eq!(log_lines, 1);
        assert_eq!(state_changed, 0);

        session.tx.send(BackendEvent::Exited(Some(0))).unwrap();
    }

    #[tokio::test]
    async fn new_connection_attempts_clear_the_previous_log_file_path() {
        let (manager, backend, profile_id, _, _) = build_manager(CredentialMode::None, None);
        let failed_session = backend.queue_session(Some(63));

        manager.connect(profile_id.to_string()).await.unwrap();
        failed_session
            .tx
            .send(BackendEvent::Stdout("AUTH_FAILED".into()))
            .unwrap();
        tokio::task::yield_now().await;
        failed_session
            .tx
            .send(BackendEvent::Exited(Some(1)))
            .unwrap();
        tokio::task::yield_now().await;
        assert!(manager.snapshot().log_file_path.is_some());

        let next_session = backend.queue_session(Some(64));
        let snapshot = manager.connect(profile_id.to_string()).await.unwrap();

        assert_eq!(snapshot.log_file_path, None);
        assert_eq!(manager.snapshot().log_file_path, None);

        next_session.tx.send(BackendEvent::Exited(Some(0))).unwrap();
    }

    #[tokio::test]
    async fn prompts_for_credentials_without_saved_username() {
        let (manager, backend, profile_id, _, _) = build_manager(CredentialMode::UserPass, None);
        let mut events = manager.subscribe();

        manager.connect(profile_id.to_string()).await.unwrap();

        assert_eq!(
            manager.snapshot().state,
            ConnectionState::AwaitingCredentials
        );
        assert_eq!(backend.request_count(), 0);

        loop {
            match events.recv().await.unwrap() {
                CoreEvent::CredentialsRequested(prompt) => {
                    assert_eq!(prompt.profile_id, profile_id);
                    assert_eq!(prompt.saved_username, None);
                    assert!(prompt.remember_supported);
                    break;
                }
                CoreEvent::StateChanged(_) | CoreEvent::LogLine(_) | CoreEvent::DnsObserved(_) => {}
            }
        }
    }

    #[tokio::test]
    async fn prompts_with_saved_username_and_does_not_autoconnect() {
        let (manager, backend, profile_id, _, _) =
            build_manager(CredentialMode::UserPass, Some("alice"));
        let mut events = manager.subscribe();

        manager.connect(profile_id.to_string()).await.unwrap();

        assert_eq!(
            manager.snapshot().state,
            ConnectionState::AwaitingCredentials
        );
        assert_eq!(backend.request_count(), 0);

        loop {
            match events.recv().await.unwrap() {
                CoreEvent::CredentialsRequested(prompt) => {
                    assert_eq!(prompt.profile_id, profile_id);
                    assert_eq!(prompt.saved_username.as_deref(), Some("alice"));
                    assert!(prompt.remember_supported);
                    break;
                }
                CoreEvent::StateChanged(_) | CoreEvent::LogLine(_) | CoreEvent::DnsObserved(_) => {}
            }
        }
    }

    #[tokio::test]
    async fn remember_username_persists_only_the_username() {
        let (manager, backend, profile_id, secret_store, repository) =
            build_manager(CredentialMode::UserPass, None);
        backend.queue_session(Some(71));

        manager.connect(profile_id.to_string()).await.unwrap();
        manager
            .submit_credentials(CredentialSubmission {
                profile_id: profile_id.clone(),
                username: "alice".into(),
                password: "secret".into(),
                remember_in_keychain: true,
            })
            .await
            .unwrap();

        let stored = secret_store.get_password(&profile_id).unwrap().unwrap();
        assert_eq!(stored.username, "alice");
        assert!(*repository.saved_credentials.lock());
    }

    #[tokio::test]
    async fn unchecked_remember_removes_saved_username() {
        let (manager, backend, profile_id, secret_store, repository) =
            build_manager(CredentialMode::UserPass, Some("alice"));
        backend.queue_session(Some(72));

        manager.connect(profile_id.to_string()).await.unwrap();
        manager
            .submit_credentials(CredentialSubmission {
                profile_id: profile_id.clone(),
                username: "bob".into(),
                password: "secret".into(),
                remember_in_keychain: false,
            })
            .await
            .unwrap();

        assert!(secret_store.get_password(&profile_id).unwrap().is_none());
        assert!(!*repository.saved_credentials.lock());
    }
}
