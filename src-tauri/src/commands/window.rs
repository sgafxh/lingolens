use crate::utils::debug_log;
use crate::utils::define_config::{get_config_from_store, save_config_to_store};
use crate::utils::show_window::{MAX_WIN_HEIGHT, MAX_WIN_WIDTH, MIN_WIN_HEIGHT, MIN_WIN_WIDTH};
use crate::utils::update::{check_for_update, UpdateCheckResponse};
use crate::AppState;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Manager, Runtime, State, WebviewWindow};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

// 更新鼠标是否在窗口内
#[tauri::command]
pub fn update_hover_status(hovered: bool, state: State<'_, AppState>) {
    state.window_locked.store(hovered, Ordering::Relaxed);
}

/// 指针是否还在窗口附近（容错 tolerance 个逻辑像素）。
///
/// "鼠标移出即隐藏"要有容错：主窗口没有原生边框，手仲向窗口边缘（尤其是去够右下角
/// 缩放手柄）时很容易擦出窗口几个像素，不该因此就把窗口关掉。
///
/// 坐标说明：`mouse_position` 在 Windows 上走 `GetCursorPos`，返回的是**物理像素**；
/// `outer_position` / `outer_size` 也是物理像素，所以这里全部在物理空间里比。
/// 拿不到信息时返回 `true`（当作"还在附近"），宁可多留一会儿也不误隐藏。
#[tauri::command]
pub fn is_cursor_near_window(window: WebviewWindow, tolerance: f64) -> bool {
    use mouse_position::mouse_position::Mouse as MousePosition;

    let Ok(pos) = window.outer_position() else {
        debug_log::log("cursor_near", "拿不到窗口位置 → 当作在附近");
        return true;
    };
    let Ok(size) = window.outer_size() else {
        debug_log::log("cursor_near", "拿不到窗口尺寸 → 当作在附近");
        return true;
    };

    let MousePosition::Position { x, y } = MousePosition::get_mouse_position() else {
        debug_log::log("cursor_near", "拿不到鼠标位置 → 当作在附近");
        return true;
    };

    let scale = window.scale_factor().unwrap_or(1.0);
    let tol = (tolerance * scale) as i32;

    let right = pos.x + size.width as i32;
    let bottom = pos.y + size.height as i32;

    let near = x >= pos.x - tol && x <= right + tol && y >= pos.y - tol && y <= bottom + tol;

    debug_log::log(
        "cursor_near",
        &format!(
            "cursor=({x},{y}) rect=({},{})-({right},{bottom}) tol={tol} near={near}",
            pos.x, pos.y
        ),
    );

    near
}

/// 前端诊断日志（写入 %TEMP%\pointra.log）
#[tauri::command]
pub fn log_debug(tag: String, msg: String) {
    debug_log::log(&tag, &msg);
}

// 保存主窗口拖拽后的尺寸
//
// 把窗口尺寸写回配置。
//
// 两条路共用：① 窗口 Resized 事件（防抖，见 utils/window_size_watcher.rs）
// ② 前端拖拽结束后主动调一次。返回 Ok(true) 表示真的写了文件。
pub fn persist_window_size<R: Runtime>(
    app: &AppHandle<R>,
    width: f64,
    height: f64,
) -> Result<bool, String> {
    if !width.is_finite() || !height.is_finite() {
        return Err("窗口尺寸非法".to_string());
    }

    let width = width.clamp(MIN_WIN_WIDTH, MAX_WIN_WIDTH);
    let height = height.clamp(MIN_WIN_HEIGHT, MAX_WIN_HEIGHT);

    let mut config = get_config_from_store(app)?;

    if (config.win_width - width).abs() < 0.5 && (config.win_height - height).abs() < 0.5 {
        debug_log::log(
            "save_size",
            &format!("尺寸未变 {width:.0}x{height:.0}，跳过写入"),
        );
        return Ok(false);
    }

    config.win_width = width;
    config.win_height = height;
    save_config_to_store(app, &config)?;

    // 同步到内存配置，避免设置窗口写回时用旧尺寸覆盖
    let state = app.state::<AppState>();
    if let Ok(mut current) = state.config.lock() {
        current.win_width = width;
        current.win_height = height;
    }

    debug_log::log(
        "save_size",
        &format!("已保存窗口尺寸 {width:.0}x{height:.0}"),
    );

    Ok(true)
}

// 保存主窗口拖拽后的尺寸
//
// 不信任前端传的数值，直接读窗口真实尺寸：拖拽中 `requestAnimationFrame` 会合帧，
// 前端手里的尺寸可能比实际值旧（之前尺寸写漏就是这么来的）。
#[tauri::command]
pub fn save_window_size(window: WebviewWindow, app: AppHandle) -> Result<bool, String> {
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    if scale <= 0.0 {
        return Err("缩放比异常".to_string());
    }

    let size = window.inner_size().map_err(|e| e.to_string())?;
    let width = size.width as f64 / scale;
    let height = size.height as f64 / scale;

    let saved = persist_window_size(&app, width, height)?;
    if saved {
        crate::utils::window_size_watcher::mark_saved(width, height);
    }

    Ok(saved)
}

// 停止快捷键
#[tauri::command]
pub fn stop_shortcuts(app: AppHandle) {
    let _ = app.global_shortcut().unregister_all();
}

// 检查更新
#[tauri::command]
pub async fn trigger_update_check(version: String) -> Result<UpdateCheckResponse, ()> {
    if let Some((update_info, has_update)) = check_for_update(&version).await {
        if has_update {
            return Ok(update_info);
        }
    }
    Err(())
}
