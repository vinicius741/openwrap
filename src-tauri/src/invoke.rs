#[macro_export]
macro_rules! invoke_handlers {
    () => {
        tauri::generate_handler![
            $crate::commands::profiles::import::import_profile,
            $crate::commands::profiles::selection::list_profiles,
            $crate::commands::profiles::selection::get_profile,
            $crate::commands::profiles::delete::delete_profile,
            $crate::commands::profiles::selection::get_last_selected_profile,
            $crate::commands::profiles::selection::set_last_selected_profile,
            $crate::commands::profiles::dns_policy::update_profile_dns_policy,
            $crate::commands::connection::connect,
            $crate::commands::connection::submit_credentials,
            $crate::commands::connection::disconnect,
            $crate::commands::connection::get_connection_state,
            $crate::commands::connection::get_recent_logs,
            $crate::commands::connection::reveal_connection_log_in_finder,
            $crate::commands::settings::get_settings,
            $crate::commands::settings::update_settings,
            $crate::commands::settings::detect_openvpn,
            $crate::commands::settings::reveal_profile_in_finder,
            $crate::commands::logs::reveal_logs_folder,
            $crate::commands::logs::get_recent_sessions,
            $crate::commands::logs::cleanup_old_logs,
        ]
    };
}
