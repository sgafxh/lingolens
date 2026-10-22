use crate::utils::define_config::{get_config_from_store, save_config_to_store, AppConfig};
use crate::utils::shortcuts::restart_shortcuts;
use crate::AppState;
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};
#[cfg(target_os = "macos")]
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


#[derive(serde::Serialize)]
pub struct TranslationSettings {
    base_url: String,
    model: String,
    target_lang: String,
    has_api_key: bool,
}

#[derive(serde::Deserialize)]
pub struct TranslationSettingsInput {
    base_url: String,
    model: String,
    target_lang: String,
    api_key: Option<String>,
}

#[tauri::command]
pub fn get_translation_settings(app: AppHandle) -> Result<TranslationSettings, String> {
    let state = app.state::<AppState>();
    let config = state.config.lock().map_err(|_| "读取设置失败")?;
    Ok(TranslationSettings {
        base_url: config.llm_base_url.clone(),
        model: config.llm_model.clone(),
        target_lang: config.llm_target_lang.clone(),
        has_api_key: !config.llm_api_key.is_empty(),
    })
}

fn apply_translation_settings(config: &AppConfig, input: TranslationSettingsInput) -> Result<AppConfig, String> {
    let base_url = input.base_url.trim().trim_end_matches('/');
    let url = reqwest::Url::parse(base_url).map_err(|_| "请输入有效的服务地址")?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none()
        || !url.username().is_empty() || url.password().is_some()
        || url.query().is_some() || url.fragment().is_some() {
        return Err("服务地址应为 HTTP(S) API 地址，密钥请填入 API Key 栏".into());
    }
    if input.model.trim().is_empty() {
        return Err("请填写模型名称".into());
    }
    if !matches!(input.target_lang.as_str(), "auto" | "zh" | "en") {
        return Err("请选择有效的目标语言".into());
    }
    let mut next = config.clone();
    next.llm_base_url = base_url.to_string();
    next.llm_model = input.model.trim().to_string();
    next.llm_target_lang = input.target_lang;
    if let Some(key) = input.api_key {
        let key = key.trim();
        if !key.is_empty() {
            next.llm_api_key = key.to_string();
        }
    }
    if next.llm_api_key.is_empty() {
        return Err("请填写 API Key".into());
    }
    Ok(next)
}

#[tauri::command]
pub fn save_translation_settings(app: AppHandle, settings: TranslationSettingsInput) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut config = state.config.lock().map_err(|_| "保存设置失败")?;
    let next = apply_translation_settings(&config, settings)?;
    save_config_to_store(&app, &next)?;
    *config = next;
    Ok(())
}

#[cfg(test)]
mod translation_settings_tests {
    use super::*;
    fn input() -> TranslationSettingsInput {
        TranslationSettingsInput { base_url: " https://example.com/v1/ ".into(), model: " test-model ".into(), target_lang: "auto".into(), api_key: None }
    }
    #[test]
    fn changing_service_preserves_key_and_other_settings() {
        let mut config = AppConfig::default();
        config.llm_api_key = "test-key".into();
        config.win_width = 640.0;
        let next = apply_translation_settings(&config, input()).unwrap();
        assert_eq!(next.llm_api_key, "test-key");
        assert_eq!(next.llm_base_url, "https://example.com/v1");
        assert_eq!(next.llm_model, "test-model");
        assert_eq!(next.win_width, 640.0);
    }
    #[test]
    fn replaces_key_and_rejects_missing_configuration() {
        let config = AppConfig::default();
        assert!(apply_translation_settings(&config, input()).is_err());
        let mut settings = input();
        settings.api_key = Some(" replacement-key ".into());
        assert_eq!(apply_translation_settings(&config, settings).unwrap().llm_api_key, "replacement-key");
    }
    #[test]
    fn rejects_invalid_address_model_and_language() {
        let mut config = AppConfig::default();
        config.llm_api_key = "test-key".into();
        for address in ["file:///tmp", "https://user:password@example.com", "https://example.com?key=secret", "not-a-url"] {
            let mut settings = input(); settings.base_url = address.into();
            assert!(apply_translation_settings(&config, settings).is_err());
        }
        let mut settings = input(); settings.model = " ".into();
        assert!(apply_translation_settings(&config, settings).is_err());
        let mut settings = input(); settings.target_lang = "invalid".into();
        assert!(apply_translation_settings(&config, settings).is_err());
    }
}
