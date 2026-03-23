use std::fs;
use std::path::PathBuf;

use chrono::Utc;

use crate::connection::SessionId;
use crate::errors::AppError;
use crate::profiles::ProfileId;

use super::model::{SessionMetadata, SessionOutcome};
use super::writer::BufferedWriter;

struct ActiveSessionLog {
    log_dir: PathBuf,
    openvpn_writer: BufferedWriter,
    dns_writer: BufferedWriter,
    core_writer: BufferedWriter,
    verbose: bool,
}

impl ActiveSessionLog {
    fn new(log_dir: PathBuf, verbose: bool) -> Result<Self, AppError> {
        fs::create_dir_all(&log_dir)?;

        let openvpn_writer = BufferedWriter::new(log_dir.join("openvpn.log"), verbose)?;
        let dns_writer = BufferedWriter::new(log_dir.join("dns.log"), verbose)?;
        let core_writer = BufferedWriter::new(log_dir.join("core.log"), verbose)?;

        Ok(Self {
            log_dir,
            openvpn_writer,
            dns_writer,
            core_writer,
            verbose,
        })
    }

    fn log_openvpn(&mut self, line: &str) {
        if let Err(e) = self.openvpn_writer.write_line(line) {
            eprintln!("[logging] Failed to write openvpn log: {}", e);
        }
    }

    fn log_dns(&mut self, line: &str) {
        if let Err(e) = self.dns_writer.write_line(line) {
            eprintln!("[logging] Failed to write dns log: {}", e);
        }
    }

    fn log_core(&mut self, line: &str) {
        if let Err(e) = self.core_writer.write_line(line) {
            eprintln!("[logging] Failed to write core log: {}", e);
        }
    }

    fn flush(&mut self) {
        let _ = self.openvpn_writer.flush();
        let _ = self.dns_writer.flush();
        let _ = self.core_writer.flush();
    }

    fn set_verbose(&mut self, verbose: bool) {
        self.verbose = verbose;
        self.openvpn_writer.set_immediate_flush(verbose);
        self.dns_writer.set_immediate_flush(verbose);
        self.core_writer.set_immediate_flush(verbose);
    }
}

pub struct SessionLogManager {
    base_logs_dir: PathBuf,
    current_session: Option<ActiveSessionLog>,
    verbose: bool,
}

impl SessionLogManager {
    pub fn new(base_logs_dir: PathBuf, verbose: bool) -> Self {
        Self {
            base_logs_dir,
            current_session: None,
            verbose,
        }
    }

    pub fn start_session(
        &mut self,
        session_id: &SessionId,
        profile_id: &ProfileId,
        profile_name: &str,
    ) -> Result<PathBuf, AppError> {
        self.end_session(SessionOutcome::Cancelled);

        let started_at = Utc::now();
        let date = started_at.format("%Y-%m-%d").to_string();
        let log_dir = self
            .base_logs_dir
            .join("sessions")
            .join(&date)
            .join(format!("session-{}", session_id));

        let mut session_log = ActiveSessionLog::new(log_dir.clone(), self.verbose)?;

        let metadata = SessionMetadata {
            session_id: session_id.to_string(),
            profile_id: profile_id.to_string(),
            profile_name: profile_name.to_string(),
            started_at,
            ended_at: None,
            outcome: SessionOutcome::InProgress,
            verbose_mode: self.verbose,
        };

        let metadata_path = log_dir.join("metadata.json");
        let metadata_json = serde_json::to_string_pretty(&metadata)
            .map_err(|e| AppError::Serialization(e.to_string()))?;
        if let Err(e) = fs::write(&metadata_path, &metadata_json) {
            eprintln!("[logging] Failed to write metadata: {}", e);
        }

        session_log.log_core(&format!(
            "[{}] Session started for profile '{}' (verbose={})",
            Utc::now().format("%Y-%m-%d %H:%M:%S%.3f"),
            profile_name,
            self.verbose
        ));

        self.current_session = Some(session_log);
        Ok(log_dir)
    }

    pub fn end_session(&mut self, outcome: SessionOutcome) {
        if let Some(mut session) = self.current_session.take() {
            session.log_core(&format!("Session ended with outcome: {:?}", outcome));

            let metadata_path = session.log_dir.join("metadata.json");
            if let Ok(content) = fs::read_to_string(&metadata_path) {
                if let Ok(mut metadata) = serde_json::from_str::<SessionMetadata>(&content) {
                    metadata.ended_at = Some(Utc::now());
                    metadata.outcome = outcome;

                    if let Ok(json) = serde_json::to_string_pretty(&metadata) {
                        if let Err(e) = fs::write(&metadata_path, &json) {
                            eprintln!("[logging] Failed to update metadata: {}", e);
                        }
                    }
                }
            }

            session.flush();
        }
    }

    pub fn log_openvpn(&mut self, line: &str) {
        if let Some(session) = &mut self.current_session {
            session.log_openvpn(line);
        }
    }

    pub fn log_dns(&mut self, line: &str) {
        if let Some(session) = &mut self.current_session {
            session.log_dns(line);
        }
    }

    pub fn log_core(&mut self, event: &str) {
        if let Some(session) = &mut self.current_session {
            session.log_core(&format!(
                "[{}] {}",
                Utc::now().format("%Y-%m-%d %H:%M:%S%.3f"),
                event
            ));
        }
    }

    pub fn flush(&mut self) {
        if let Some(session) = &mut self.current_session {
            session.flush();
        }
    }

    pub fn current_session_dir(&self) -> Option<&PathBuf> {
        self.current_session.as_ref().map(|s| &s.log_dir)
    }

    pub fn set_verbose(&mut self, verbose: bool) {
        self.verbose = verbose;
        if let Some(session) = &mut self.current_session {
            session.set_verbose(verbose);
        }
    }

    pub fn get_base_logs_dir(&self) -> &PathBuf {
        &self.base_logs_dir
    }
}
