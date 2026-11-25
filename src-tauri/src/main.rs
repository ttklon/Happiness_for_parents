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

// A GUI-only Windows process has no console: without this file a silent exit
// was impossible to diagnose. These helpers run before Tauri starts.
fn diagnostic_file() -> std::path::PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("com.happiness.scanner").join("startup.log")
}

fn startup_log(message: &str) {
    use std::io::Write;
    let path = diagnostic_file();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(
            file,
            "{} {}",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            message
        );
    }
}

#[cfg(target_os = "windows")]
fn show_startup_error(message: &str) {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    fn wide(text: &str) -> Vec<u16> {
        OsStr::new(text)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            wide(message).as_ptr(),
            wide("Happiness Scanner — ошибка запуска").as_ptr(),
            0x10, // MB_ICONERROR
        );
    }
}

#[cfg(target_os = "windows")]
#[link(name = "user32")]
extern "system" {
    fn MessageBoxW(
        hwnd: *mut std::ffi::c_void,
        text: *const u16,
        caption: *const u16,
        kind: u32,
    ) -> i32;
}

#[cfg(not(target_os = "windows"))]
fn show_startup_error(message: &str) {
    eprintln!("{message}");
}

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
    startup_log(&format!(
        "Запуск v{}; exe={:?}; cwd={:?}; args={:?}",
        env!("CARGO_PKG_VERSION"),
        std::env::current_exe(),
        std::env::current_dir(),
        std::env::args().collect::<Vec<_>>()
    ));
    let result = tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .build(),
        )
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
            let handle = app.handle();
            let mut cfg = config::load(&handle).unwrap_or_default();
            startup_log("setup: конфиг загружен");

            // v0.6.1 hid the very first window ("stealth by default"), so an
            // ordinary install looked like it never launched. Show the window
            // once after upgrading, then respect the user's own settings.
            if let Ok(marker) = config::data_dir(&handle)
                .map(|dir| dir.join("visible-on-startup-v0.6.4"))
            {
                if !marker.exists() {
                    startup_log("первый запуск после обновления: окно будет видимым");
                    cfg.start_minimized = false;
                    cfg.stealth_window_title = false;
                    if let Err(e) = config::save(&handle, &cfg) {
                        startup_log(&format!("не удалось сохранить конфиг: {e}"));
                    }
                    if let Err(e) = std::fs::write(&marker, b"1") {
                        startup_log(&format!("не удалось создать метку миграции: {e}"));
                    }
                }
            }

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
            startup_log("setup: трей создан");

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
            let args: Vec<String> = std::env::args().collect();
            let force_visible = args.iter().any(|a| a == "--show" || a == "--force-visible");
            let minimized = !force_visible
                && (cfg.start_minimized || args.iter().any(|arg| arg == "--minimized"));
            if minimized {
                let _ = window.hide();
                startup_log("окно скрыто в трей по настройке start_minimized");
            } else {
                let _ = window.show();
                let _ = window.set_focus();
                startup_log("главное окно показано");
            }

            *state.config.lock().map_err(|e| e.to_string())? = cfg;
            state.log("info", "Happiness Scanner готов. Нажмите «СТАРТ» — остальное автоматизировано.");
            startup_log("setup: готово, вход в event loop");
            Ok(())
        })
        .run(tauri::generate_context!());

    if let Err(err) = result {
        startup_log(&format!("ФАТАЛЬНАЯ ОШИБКА: {err}"));
        show_startup_error(&format!(
            "Не удалось запустить Happiness Scanner.\n\n{err}\n\n\
            Если в тексте ошибки нет указания на конкретный файл, проверьте\
            Microsoft Edge WebView2 Runtime: https://aka.ms/webview2\n\n\
            Подробности: {}",
            diagnostic_file().display()
        ));
    }
}
