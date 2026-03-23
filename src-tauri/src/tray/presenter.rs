use tauri::Manager;

use openwrap_core::connection::{ConnectionSnapshot, ConnectionState};

use super::menu::{TrayState, TRAY_ID};
use crate::app_state::AppState;

pub fn apply_tray_state(
    app: &tauri::AppHandle,
    snapshot: &ConnectionSnapshot,
    target_name: Option<&str>,
) {
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let tooltip = match (snapshot.state.clone(), target_name) {
            (ConnectionState::Connected, Some(name)) => format!("Connected: {name}"),
            (ConnectionState::Connecting | ConnectionState::Reconnecting, Some(name)) => {
                format!("Connecting: {name}")
            }
            (ConnectionState::AwaitingCredentials, Some(name)) => {
                format!("Credentials required: {name}")
            }
            (ConnectionState::Error, Some(name)) => format!("Connection failed: {name}"),
            (_, Some(name)) => format!("Ready: {name}"),
            _ => "OpenWrap".into(),
        };
        let _ = tray.set_tooltip(Some(tooltip));
    }

    if let Some(state) = app.try_state::<TrayState>() {
        let connect_label = target_name
            .map(|name| format!("Connect {name}"))
            .unwrap_or_else(|| "Connect".into());
        let _ = state.connect.set_text(&connect_label);

        let can_connect = target_name.is_some()
            && matches!(
                snapshot.state,
                ConnectionState::Idle | ConnectionState::Error
            );
        let can_disconnect = matches!(
            snapshot.state,
            ConnectionState::Connecting
                | ConnectionState::Connected
                | ConnectionState::Reconnecting
                | ConnectionState::AwaitingCredentials
        );
        let _ = state.connect.set_enabled(can_connect);
        let _ = state.disconnect.set_enabled(can_disconnect);
    }
}

pub fn sync_connection_state(app: &tauri::AppHandle, snapshot: &ConnectionSnapshot) {
    let target_name = app.try_state::<AppState>().and_then(|state| {
        crate::tray::target::resolve_connect_target(&state).map(|(_, name)| name)
    });
    apply_tray_state(app, snapshot, target_name.as_deref());
}

pub fn sync_selected_profile(
    app: &tauri::AppHandle,
    profile_id: Option<&openwrap_core::profiles::ProfileId>,
) {
    let target_name = profile_id.and_then(|profile_id| {
        let state = app.try_state::<AppState>()?;
        state
            .profile_repository()
            .get_profile(profile_id)
            .ok()
            .map(|profile| profile.profile.name)
    });
    let snapshot = app
        .try_state::<AppState>()
        .map(|state| state.connection_manager.snapshot())
        .unwrap_or_default();
    apply_tray_state(app, &snapshot, target_name.as_deref());
}
