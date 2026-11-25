#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod balance;
mod config;
mod crypto;
mod notifier;
mod results;
mod scanner;
mod state;
mod updater;
mod wallet;

use config::AppConfig;
use state::{AppState, LogEvent, ScanStats};
use std::sync::Arc;
use tauri::{Manager, State};

// ---------------------------------------------------------------------------
// Commands exposed to the React UI
// ---------------------------------------------------------------------------

#[tauri::command]
fn get_stats(state: State<'_, Arc<AppState>>) -> Result<ScanStats, String> {
    Ok(state.stats.lock().map_err(|e| e.to_string())?.clone())
}

#[tauri::command]
fn get_events(state: State<'_, Arc<AppState>>) -> Result<Vec<LogEvent>, String> {
    Ok(state.events.lock().map_err(|e| e.to_string())?.iter().cloned().collect())
}

#[tauri::command]
fn get_history(state: State<'_, Arc<AppState>>) -> Result<Vec<u64>, String> {
    Ok(state.history.lock().map_err(|e| e.to_string())?.iter().copied().collect())
}

#[tauri::command]
fn get_hits(state: State<'_, Arc<AppState>>) -> Result<Vec<results::HitRecord>, String> {
    Ok(state.hits.lock().map_err(|e| e.to_string())?.clone())
}

#[tauri::command]
fn get_stored_hits(state: State<'_, Arc<AppState>>, handle: tauri::AppHandle) -> Result<Vec<results::HitRecord>, String> {
    let cfg = state.config.lock().map_err(|e| e.to_string())?.clone();
    let password = if cfg.encryption_enabled && !cfg.encryption_password.is_empty() {
        Some(cfg.encryption_password.as_str())
    } else {
        None
    };
    let path = config::results_dir(&handle)?.join("hits.jsonl");
    results::read_hits(&path, password)
}

#[tauri::command]
fn clear_hits(state: State<'_, Arc<AppState>>, handle: tauri::AppHandle) -> Result<String, String> {
    state.hits.lock().map_err(|e| e.to_string())?.clear();
    let path = config::results_dir(&handle)?.join("hits.jsonl");
    results::clear_file(&path)?;
    Ok("Результаты очищены".into())
}

#[tauri::command]
fn load_config(state: State<'_, Arc<AppState>>, handle: tauri::AppHandle) -> Result<AppConfig, String> {
    let cfg = config::load(&handle)?;
    *state.config.lock().map_err(|e| e.to_string())? = cfg.clone();
    Ok(cfg)
}

#[tauri::command]
fn save_config(state: State<'_, Arc<AppState>>, handle: tauri::AppHandle, config: AppConfig) -> Result<Vec<String>, String> {
    let warnings = config::save(&handle, &config)?;
    *state.config.lock().map_err(|e| e.to_string())? = config.normalized();
    Ok(warnings)
}

#[tauri::command]
fn export_config(config: AppConfig) -> Result<String, String> {
    serde_json::to_string_pretty(&config.normalized()).map_err(|e| e.to_string())
}

#[tauri::command]
fn import_config(json: String) -> Result<AppConfig, String> {
    let cfg: AppConfig = serde_json::from_str(&json).map_err(|e| format!("Неверный JSON: {e}"))?;
    Ok(cfg.normalized())
}

#[tauri::command]
async fn start_scanner(
    handle: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    config: AppConfig,
) -> Result<String, String> {
    scanner::start(handle, state.inner().clone(), config)
}

#[tauri::command]
fn stop_scanner(state: State<'_, Arc<AppState>>) -> Result<String, String> {
    scanner::stop(state.inner().clone())
}

#[tauri::command]
async fn test_api(config: AppConfig) -> Result<String, String> {
    balance::probe(&config.normalized()).await
}

fn updater_proxy(state: &State<'_, Arc<AppState>>) -> Result<Option<String>, String> {
    let cfg = state.config.lock().map_err(|e| e.to_string())?;
    if cfg.block_direct_connection && cfg.proxy().is_none() {
        return Err("Блок прямого соединения: настройте Tor или прокси перед проверкой обновлений".into());
    }
    Ok(cfg.proxy())
}

#[tauri::command]
async fn check_for_updates(state: State<'_, Arc<AppState>>) -> Result<updater::UpdateInfo, String> {
    updater::check(updater_proxy(&state)?).await
}

#[tauri::command]
async fn install_update(
    state: State<'_, Arc<AppState>>,
    info: updater::UpdateInfo,
) -> Result<String, String> {
    updater::download_and_install(&info, updater_proxy(&state)?).await
}

#[tauri::command]
fn endpoint_health() -> Result<Vec<balance::EndpointHealth>, String> {
    Ok(balance::health_snapshot())
}

#[tauri::command]
fn preview_candidates(config: AppConfig, count: Option<usize>) -> Result<Vec<(String, String, String)>, String> {
    Ok(scanner::preview_candidates(&config, count.unwrap_or(3).clamp(1, 10)))
}

#[tauri::command]
fn set_window_visible(handle: tauri::AppHandle, visible: bool) -> Result<String, String> {
    if let Some(window) = handle.get_webview_window("main") {
        if visible {
            window.show().map_err(|e| e.to_string())?;
            window.set_focus().map_err(|e| e.to_string())?;
        } else {
            window.hide().map_err(|e| e.to_string())?;
        }
    }
    Ok("ok".into())
}

#[tauri::command]
fn set_autostart(handle: tauri::AppHandle, enabled: bool) -> Result<String, String> {
    config::set_autostart(enabled)?;
    let mut cfg = config::load(&handle)?;
    cfg.autostart = enabled;
    config::save(&handle, &cfg)?;
    Ok(if enabled {
        "Автозапуск включён".into()
    } else {
        "Автозапуск выключен".into()
    })
}

#[tauri::command]
fn set_stealth_title(handle: tauri::AppHandle, enabled: bool) -> Result<String, String> {
    let title = if enabled {
        "System Host Process"
    } else {
        "Happiness Scanner"
    };
    if let Some(window) = handle.get_webview_window("main") {
        window.set_title(title).map_err(|e| e.to_string())?;
    }
    Ok("ok".into())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::default().build())
        .manage(Arc::new(AppState::default()))
        .invoke_handler(tauri::generate_handler![
            get_stats,
            get_events,
            get_history,
            get_hits,
            get_stored_hits,
            clear_hits,
            load_config,
            save_config,
            export_config,
            import_config,
            start_scanner,
            stop_scanner,
            test_api,
            endpoint_health,
            check_for_updates,
            install_update,
            preview_candidates,
            set_window_visible,
            set_autostart,
            set_stealth_title,
        ])
        .setup(|app| {
            let state = app.state::<Arc<AppState>>().inner().clone();
            let cfg = config::load(&app.handle()).unwrap_or_default();

            // Tray icon — the app keeps scanning after the window is closed.
            let show = tauri::menu::MenuItem::with_id(app, "show", "Показать окно", true, None::<&str>)?;
            let hide = tauri::menu::MenuItem::with_id(app, "hide", "Скрыть в трей", true, None::<&str>)?;
            let quit = tauri::menu::MenuItem::with_id(app, "quit", "Выход", true, None::<&str>)?;
            let menu = tauri::menu::Menu::with_items(app, &[&show, &hide, &quit])?;
            let tray = tauri::tray::TrayIconBuilder::new()
                .icon(app.default_window_icon().cloned().expect("bundled icon"))
                .menu(&menu)
                .tooltip("Happiness Scanner — фоновый поиск")
                .menu_on_left_click(true)
                .on_menu_event(move |app, event| match event.id().0.as_str() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "hide" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.hide();
                        }
                    }
                    "quit" => {
                        let _ = scanner::stop(app.state::<Arc<AppState>>().inner().clone());
                        std::process::exit(0);
                    }
                    _ => {}
                })
                .build(app)?;
            let _tray = tray;

            let window = app.get_webview_window("main").expect("main window");
            let window_clone = window.clone();
            window.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window_clone.hide();
                }
            });

            if cfg.stealth_window_title {
                let _ = window.set_title("System Host Process");
            }
            let minimized = cfg.start_minimized
                || std::env::args().any(|arg| arg == "--minimized");
            if minimized {
                let _ = window.hide();
            }

            *state.config.lock().map_err(|e| e.to_string())? = cfg;
            state.log("info", "Happiness Scanner готов. Нажмите «СТАРТ» — остальное автоматизировано.");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Happiness Scanner");
}
