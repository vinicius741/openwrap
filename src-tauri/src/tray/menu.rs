use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{include_image, Emitter, Wry};

use crate::events::TRAY_ACTION;

pub const TRAY_ID: &str = "openwrap-tray";
pub const SHOW_ID: &str = "show";
pub const CONNECT_ID: &str = "connect";
pub const DISCONNECT_ID: &str = "disconnect";
pub const QUIT_ID: &str = "quit";

pub type AppMenuItem = MenuItem<Wry>;

pub struct TrayState {
    pub connect: AppMenuItem,
    pub disconnect: AppMenuItem,
}

pub fn build_tray_menu(app: &tauri::AppHandle) -> tauri::Result<(Menu<Wry>, TrayState)> {
    let show = MenuItem::with_id(app, SHOW_ID, "Show OpenWrap", true, None::<&str>)?;
    let connect = MenuItem::with_id(app, CONNECT_ID, "Connect", true, None::<&str>)?;
    let disconnect = MenuItem::with_id(app, DISCONNECT_ID, "Disconnect", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, QUIT_ID, "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &connect, &disconnect, &quit])?;

    let state = TrayState {
        connect: connect.clone(),
        disconnect: disconnect.clone(),
    };

    Ok((menu, state))
}

pub fn attach_tray_icon(
    app: &tauri::AppHandle,
    menu: &Menu<Wry>,
) -> tauri::Result<tauri::tray::TrayIcon> {
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(include_image!("icons/32x32.png"))
        .icon_as_template(true)
        .menu(menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| {
            let id = event.id().as_ref();
            if id == SHOW_ID {
                crate::tray::actions::show_window(app);
                let _ = app.emit(TRAY_ACTION, "show");
            } else if id == CONNECT_ID {
                crate::tray::actions::handle_connect(app);
                let _ = app.emit(TRAY_ACTION, "connect");
            } else if id == DISCONNECT_ID {
                crate::tray::actions::handle_disconnect(app);
                let _ = app.emit(TRAY_ACTION, "disconnect");
            } else if id == QUIT_ID {
                app.exit(0);
            }
        })
        .build(app)
}
