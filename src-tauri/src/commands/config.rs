use crate::utils::define_config::{get_config_from_store, save_config_to_store, AppConfig};
use crate::utils::shortcuts::restart_shortcuts;
use crate::AppState;
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_autostart::ManagerExt;

#[tauri::command]
pub async fn get_config(app: AppHandle) -> Result<AppConfig, String> {
    get_config_from_store(&app)
}

#[tauri::command]
pub async fn update_config(app: AppHandle, mut new_config: AppConfig) -> Result<bool, String> {
    // 主窗口尺寸由右下角拖拽手柄维护（save_window_size），设置页不管理这个字段。
    // 设置页提交的是它自己那份快照，直接保存会用旧尺寸盖掉用户刚拖好的大小。
    // 翻译后端三个字段同理：设置页不提供编辑，必须保留当前值（否则会抹掉 API Key）。
    if let Ok(current) = app.state::<AppState>().config.lock() {
        new_config.win_width = current.win_width;
        new_config.win_height = current.win_height;
        new_config.llm_base_url = current.llm_base_url.clone();
        new_config.llm_model = current.llm_model.clone();
        new_config.llm_target_lang = current.llm_target_lang.clone();
        new_config.llm_api_key = current.llm_api_key.clone();
    }

    save_config_to_store(&app, &new_config)?;
    restart_shortcuts(&app, new_config.clone());

    // macOS: 根据 auto_start 启用或禁用开机自启
    #[cfg(target_os = "macos")]
    {
        let autostart_manager = app.autolaunch();
        if new_config.auto_start {
            if !autostart_manager.is_enabled().unwrap_or(false) {
                autostart_manager
                    .enable()
                    .map_err(|e| format!("启用开机自启失败: {}", e))?;
            }
        } else {
            if autostart_manager.is_enabled().unwrap_or(false) {
                autostart_manager
                    .disable()
                    .map_err(|e| format!("禁用开机自启失败: {}", e))?;
            }
        }
    }

    // 传播new_config
    app.emit("win-event", json!({"type": "config", "data": new_config}))
        .ok();
    Ok(true)
}

#[tauri::command]
pub async fn complete_setup(app: AppHandle) -> Result<bool, String> {
    let mut config = get_config_from_store(&app).unwrap_or_default();
    config.first_setup_completed = true;
    save_config_to_store(&app, &config)?;

    // 重启应用（此调用不会返回)
    restart_app()
}

/// 重启当前应用
fn restart_app() -> ! {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe).spawn();
    }
    std::process::exit(0);
}
