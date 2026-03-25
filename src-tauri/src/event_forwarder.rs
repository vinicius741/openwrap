use crate::app_state::AppState;
use openwrap_core::connection::CoreEvent;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::broadcast::error::RecvError;

pub fn spawn_event_forwarder(app: AppHandle) {
    let events_app = app.clone();
    let mut event_rx = app.state::<AppState>().connection_manager.subscribe();

    tauri::async_runtime::spawn(async move {
        loop {
            let event = match event_rx.recv().await {
                Ok(event) => event,
                Err(RecvError::Lagged(skipped)) => {
                    eprintln!(
                        "warning: connection event forwarder lagged and skipped {skipped} events"
                    );
                    continue;
                }
                Err(RecvError::Closed) => break,
            };

            match event {
                CoreEvent::StateChanged(payload) => {
                    crate::tray::sync_connection_state(&events_app, &payload);
                    let _ = events_app.emit(crate::events::CONNECTION_STATE_CHANGED, payload);
                }
                CoreEvent::LogLine(payload) => {
                    let _ = events_app.emit(crate::events::CONNECTION_LOG_LINE, payload);
                }
                CoreEvent::CredentialsRequested(payload) => {
                    let _ =
                        events_app.emit(crate::events::CONNECTION_CREDENTIALS_REQUESTED, payload);
                }
                CoreEvent::DnsObserved(payload) => {
                    let _ = events_app.emit(crate::events::CONNECTION_DNS_OBSERVED, payload);
                }
            }
        }
    });
}
