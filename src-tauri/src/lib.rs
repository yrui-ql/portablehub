pub mod cli;
pub mod commands;
pub mod domain;
pub mod infra;
pub mod selftest;

use commands::AppState;
use std::path::PathBuf;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        commands::toggle_main_window(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            let exe = std::env::current_exe().unwrap_or_default();
            let root = exe.parent().map(PathBuf::from).unwrap_or_default();
            let loaded = domain::config::load(&root);
            let cfg = loaded.config.clone();
            let warning = loaded.warning.clone();

            let _ = std::fs::create_dir_all(root.join("_cache").join("webview2"));
            let _ = std::fs::create_dir_all(root.join("Themes"));
            let _ = std::fs::create_dir_all(root.join(&cfg.apps_dir));

            app.manage(AppState::new(root.clone(), cfg.clone(), warning));

            let mut builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                .title(domain::identity::app_name())
                .inner_size(1000.0, 660.0)
                .min_inner_size(760.0, 520.0)
                .decorations(false)
                .position(120.0, 60.0)
                .visible(false)
                .data_directory(root.join("_cache").join("webview2"));
            // BUG-0021: without an explicit icon the taskbar button is blank.
            if let Some(icon) = app.default_window_icon().cloned() {
                builder = builder.icon(icon)?;
            }
            let window = builder.build()?;
            if let Some(icon) = app.default_window_icon().cloned() {
                let _ = window.set_icon(icon);
            }
            let _ = window.unminimize();

            // Safety net: if the frontend never signals readiness, still show the
            // window so the user is not left with an invisible process.
            let fallback = window.clone();
            let hidden_at_login = std::env::args().any(|a| a == infra::autostart::AUTOSTART_ARG)
                && cfg.start_hidden_at_login;
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(3000));
                if !hidden_at_login {
                    let _ = fallback.unminimize();
                    if let Ok(false) = fallback.is_visible() {
                        let _ = fallback.show();
                    }
                }
            });

            let w = window.clone();
            window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    let close_to_tray = w.app_handle().state::<AppState>().cfg().close_to_tray;
                    if close_to_tray {
                        api.prevent_close();
                        let _ = w.hide();
                    }
                }
            });

            let toggle = MenuItem::with_id(app, "toggle", "显示 / 隐藏", true, None::<&str>)?;
            let run_now = MenuItem::with_id(app, "run", "立即运行自启项", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&toggle, &run_now, &quit])?;

            let tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "toggle" => commands::toggle_main_window(app),
                    "run" => {
                        let state = app.state::<AppState>();
                        let _ = infra::autostart::launch_flagged(&state.root, &state.cfg(), false);
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        commands::toggle_main_window(tray.app_handle());
                    }
                })
                .build(app)?;
            std::mem::forget(tray);

            if let Err(e) = app.global_shortcut().register(cfg.hotkey.as_str()) {
                log::warn!("global shortcut registration failed: {e}");
            }

            // "Autostart" entries mean "ensure these are running whenever the hub
            // starts", whether that is Windows login or a manual launch. Each entry
            // is checked against the process list first, so this never spawns a
            // duplicate of something the user already opened.
            let state_root = root.clone();
            let state_cfg = cfg.clone();
            std::thread::spawn(move || {
                let _ = infra::autostart::launch_flagged(&state_root, &state_cfg, false);
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap,
            commands::list_apps,
            commands::scan_apps,
            commands::discover_apps,
            commands::cancel_discover,
            commands::list_drives,
            commands::import_apps,
            commands::add_apps,
            commands::audit_apps,
            commands::export_bundle,
            commands::ai_get_config,
            commands::ai_save_config,
            commands::ai_set_consent,
            commands::ai_test_connection,
            commands::ai_detect_clis,
            commands::ai_bind_cli,
            commands::classify_apps,
            commands::classify_discovered,
            commands::apply_categories,
            commands::update_app,
            commands::remove_app,
            commands::set_tag_style,
            commands::launch_app,
            commands::reveal_app,
            commands::open_theme_dir,
            commands::get_icons,
            commands::list_themes,
            commands::get_theme,
            commands::set_theme,
            commands::set_hotkey,
            commands::suspend_hotkey,
            commands::resume_hotkey,
            commands::set_launch_at_login,
            commands::get_startup_status,
            commands::set_app_autostart,
            commands::set_apps_dir,
            commands::set_import_mode,
            commands::set_locale,
            commands::set_close_to_tray,
            commands::set_pixel_icons,
            commands::set_pixel_font,
            commands::set_discover_roots,
            commands::save_collections,
            commands::set_favorites,
            commands::set_start_hidden_at_login,
            commands::set_app_tags,
            commands::run_autostart_now,
            commands::show_main_window,
            commands::hide_main_window,
            commands::quit_app,
            commands::log_frontend,
        ])
        .run(tauri::generate_context!())
        .expect("failed to start the application");
}
