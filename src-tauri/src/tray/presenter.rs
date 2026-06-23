use tauri::{include_image, Manager};

use openwrap_core::connection::{ConnectionSnapshot, ConnectionState};

use super::menu::{TrayState, TRAY_ID};
use crate::app_state::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrayVisualState {
    Disconnected,
    Connecting,
    Connected,
}

impl TrayVisualState {
    fn from_connection_state(state: &ConnectionState) -> Self {
        match state {
            ConnectionState::Connected => Self::Connected,
            ConnectionState::Idle | ConnectionState::Error => Self::Disconnected,
            _ => Self::Connecting,
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Disconnected => "VPN OFF",
            Self::Connecting => "VPN …",
            Self::Connected => "VPN ON",
        }
    }

    fn menu_status(self) -> &'static str {
        match self {
            Self::Disconnected => "VPN: Disconnected",
            Self::Connecting => "VPN: Connecting…",
            Self::Connected => "VPN: Connected",
        }
    }
}

pub fn apply_tray_state(
    app: &tauri::AppHandle,
    snapshot: &ConnectionSnapshot,
    target_name: Option<&str>,
) {
    let visual_state = TrayVisualState::from_connection_state(&snapshot.state);

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
        let _ = tray.set_title(Some(visual_state.title()));
        let icon = match visual_state {
            TrayVisualState::Disconnected => include_image!("icons/tray-disconnected.png"),
            TrayVisualState::Connecting => include_image!("icons/tray-connecting.png"),
            TrayVisualState::Connected => include_image!("icons/tray-connected.png"),
        };
        let _ = tray.set_icon(Some(icon));
    }

    if let Some(state) = app.try_state::<TrayState>() {
        let _ = state.status.set_text(visual_state.menu_status());
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

#[cfg(test)]
mod tests {
    use super::TrayVisualState;
    use openwrap_core::connection::ConnectionState;

    #[test]
    fn connected_is_the_only_on_state() {
        let states = [
            ConnectionState::Idle,
            ConnectionState::ValidatingProfile,
            ConnectionState::AwaitingCredentials,
            ConnectionState::PreparingRuntime,
            ConnectionState::StartingProcess,
            ConnectionState::Connecting,
            ConnectionState::Reconnecting,
            ConnectionState::Disconnecting,
            ConnectionState::Error,
        ];

        for state in states {
            assert_ne!(
                TrayVisualState::from_connection_state(&state),
                TrayVisualState::Connected
            );
        }
        assert_eq!(
            TrayVisualState::from_connection_state(&ConnectionState::Connected),
            TrayVisualState::Connected
        );
    }

    #[test]
    fn idle_and_error_are_visually_disconnected() {
        assert_eq!(
            TrayVisualState::from_connection_state(&ConnectionState::Idle),
            TrayVisualState::Disconnected
        );
        assert_eq!(
            TrayVisualState::from_connection_state(&ConnectionState::Error),
            TrayVisualState::Disconnected
        );
    }
}
