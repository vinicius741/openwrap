// Tray icon configuration uses macOS template icons (icon_as_template).
// Title text is intentionally omitted as it's not standard for macOS tray icons
// and the tooltip provides sufficient status information across all platforms.

use tauri::include_image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager};

use openwrap_core::connection::{ConnectionSnapshot, ConnectionState};
use openwrap_core::profiles::ProfileId;

use crate::app_state::AppState;
use crate::events::TRAY_ACTION;

const TRAY_ID: &str = "openwrap-tray";
const SHOW_ID: &str = "show";
const CONNECT_ID: &str = "connect";
const DISCONNECT_ID: &str = "disconnect";
const QUIT_ID: &str = "quit";

type AppMenuItem = MenuItem<tauri::Wry>;

pub struct TrayState {
    connect: AppMenuItem,
    disconnect: AppMenuItem,
}

pub fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, SHOW_ID, "Show OpenWrap", true, None::<&str>)?;
    let connect = MenuItem::with_id(app, CONNECT_ID, "Connect", true, None::<&str>)?;
    let disconnect = MenuItem::with_id(app, DISCONNECT_ID, "Disconnect", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, QUIT_ID, "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &connect, &disconnect, &quit])?;

    app.manage(TrayState {
        connect: connect.clone(),
        disconnect: disconnect.clone(),
    });

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(include_image!("icons/32x32.png"))
        .icon_as_template(true)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            SHOW_ID => {
                show_window(app);
                let _ = app.emit(TRAY_ACTION, "show");
            }
            CONNECT_ID => {
                let app_handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    let state = app_handle.state::<AppState>();
                    if let Some((profile_id, _)) = resolve_connect_target(&state) {
                        let _ = state
                            .connection_manager
                            .connect(profile_id.to_string())
                            .await;
                    }
                });
                let _ = app.emit(TRAY_ACTION, "connect");
            }
            DISCONNECT_ID => {
                let app_handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    let state = app_handle.state::<AppState>();
                    let _ = state.connection_manager.disconnect().await;
                });
                let _ = app.emit(TRAY_ACTION, "disconnect");
            }
            QUIT_ID => app.exit(0),
            _ => {}
        })
        .build(app)?;

    sync_connection_state(app, &app.state::<AppState>().connection_manager.snapshot());
    Ok(())
}

pub fn sync_connection_state(app: &AppHandle, snapshot: &ConnectionSnapshot) {
    let target_name = app
        .try_state::<AppState>()
        .and_then(|state| resolve_connect_target(&state).map(|(_, name)| name));
    apply_tray_state(app, snapshot, target_name.as_deref());
}

pub fn sync_selected_profile(app: &AppHandle, profile_id: Option<&ProfileId>) {
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

fn apply_tray_state(app: &AppHandle, snapshot: &ConnectionSnapshot, target_name: Option<&str>) {
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

fn resolve_connect_target(state: &tauri::State<'_, AppState>) -> Option<(ProfileId, String)> {
    let repository = state.profile_repository();
    repository
        .get_last_selected_profile()
        .ok()
        .flatten()
        .and_then(|profile_id| {
            repository
                .get_profile(&profile_id)
                .ok()
                .map(|profile| (profile_id, profile.profile.name))
        })
        .or_else(|| {
            repository
                .list_profiles()
                .ok()?
                .into_iter()
                .next()
                .map(|profile| (profile.id, profile.name))
        })
}

fn show_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}
