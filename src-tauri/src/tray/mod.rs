use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager};

use crate::app_state::AppState;
use crate::events::TRAY_ACTION;

const SHOW_ID: &str = "show";
const CONNECT_ID: &str = "connect";
const DISCONNECT_ID: &str = "disconnect";
const QUIT_ID: &str = "quit";

pub fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, SHOW_ID, "Show OpenWrap", true, None::<&str>)?;
    let connect = MenuItem::with_id(app, CONNECT_ID, "Connect", true, None::<&str>)?;
    let disconnect = MenuItem::with_id(app, DISCONNECT_ID, "Disconnect", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, QUIT_ID, "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &connect, &disconnect, &quit])?;

    TrayIconBuilder::with_id("openwrap-tray")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            SHOW_ID => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
                let _ = app.emit(TRAY_ACTION, "show");
            }
            CONNECT_ID => {
                let app_handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    let state = app_handle.state::<AppState>();
                    let target = state
                        .profile_repository()
                        .get_last_selected_profile()
                        .ok()
                        .flatten()
                        .or_else(|| state.profile_repository().list_profiles().ok()?.first().map(|profile| profile.id.clone()));
                    if let Some(profile_id) = target {
                        let _ = state.connection_manager.connect(profile_id.to_string()).await;
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

    Ok(())
}

