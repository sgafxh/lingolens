use mouse_position::mouse_position::Mouse as MousePosition;
use serde_json::json;
use tauri::{Emitter, LogicalSize, PhysicalPosition, PhysicalSize, Runtime, Size, WebviewWindow};

use crate::utils::debug_log;

/// 最小窗口尺寸（逻辑像素）：与 tauri.conf.json / 前端 ResizeHandle 保持一致
pub const MIN_WIN_WIDTH: f64 = 260.0;
pub const MIN_WIN_HEIGHT: f64 = 200.0;
/// 尺寸上限（逻辑像素）：兜底用，正常由前端按显示器工作区比例约束
pub const MAX_WIN_WIDTH: f64 = 2400.0;
pub const MAX_WIN_HEIGHT: f64 = 1800.0;
/// 默认窗口尺寸（逻辑像素）：与 tauri.conf.json 保持一致
const DEFAULT_WIN_WIDTH: f64 = 300.0;
const DEFAULT_WIN_HEIGHT: f64 = 420.0;
/// 窗口与光标之间的间距（逻辑像素）
const CURSOR_GAP: f64 = 10.0;
/// 窗口与显示器工作区边缘保留的间距（逻辑像素），避免贴着任务栏或屏幕边
const EDGE_MARGIN: f64 = 8.0;
/// 最大不超过显示器工作区的比例
const MAX_MONITOR_RATIO: f64 = 0.9;

/// 当前显示器上允许的最大逻辑尺寸
fn monitor_max_logical<R: Runtime>(window: &WebviewWindow<R>) -> (f64, f64) {
    let fallback = (MAX_WIN_WIDTH, MAX_WIN_HEIGHT);

    let Ok(Some(monitor)) = window.current_monitor() else {
        return fallback;
    };

    let scale = monitor.scale_factor();
    let size = monitor.size();

    (
        (size.width as f64 / scale * MAX_MONITOR_RATIO).min(MAX_WIN_WIDTH),
        (size.height as f64 / scale * MAX_MONITOR_RATIO).min(MAX_WIN_HEIGHT),
    )
}

/// 应用上次拖拽保存的窗口尺寸（启动时调用一次）
pub fn apply_saved_window_size<R: Runtime>(window: &WebviewWindow<R>, width: f64, height: f64) {
    let (max_w, max_h) = monitor_max_logical(window);
    let w = width.clamp(MIN_WIN_WIDTH, max_w.max(MIN_WIN_WIDTH));
    let h = height.clamp(MIN_WIN_HEIGHT, max_h.max(MIN_WIN_HEIGHT));

    // 系统层也兜住最小值，避免前端逻辑失效时窗口被拖成一条缝
    let _ = window.set_min_size(Some(Size::Logical(LogicalSize::new(
        MIN_WIN_WIDTH,
        MIN_WIN_HEIGHT,
    ))));
    let _ = window.set_size(Size::Logical(LogicalSize::new(w, h)));
}

pub fn show_main_window<R: Runtime>(window: &WebviewWindow<R>, custom_x: i32, custom_y: i32) {
    if let MousePosition::Position { x, y } = MousePosition::get_mouse_position() {
        let monitors = window.available_monitors().unwrap_or_default();

        let monitor = monitors.iter().find(|m| {
            let pos = m.position();
            let size = m.size();

            // 光标坐标是物理像素，直接用物理边界判断。
            // （原来把物理坐标跟逻辑边界比，175% 缩放下光标一过逻辑宽度就找不到显示器了）
            x >= pos.x
                && x < pos.x + size.width as i32
                && y >= pos.y
                && y < pos.y + size.height as i32
        });

        let monitor = match monitor.or_else(|| monitors.first()) {
            Some(m) => m,
            None => return,
        };

        let scale = monitor.scale_factor();

        // 用窗口的真实尺寸参与边界计算：窗口现在可以自由缩放了，
        // 再写死 320x420 会让贴边定位算错。
        let window_size = window.inner_size().unwrap_or_else(|_| {
            PhysicalSize::new(
                (DEFAULT_WIN_WIDTH * scale) as u32,
                (DEFAULT_WIN_HEIGHT * scale) as u32,
            )
        });
        let window_phys_w = window_size.width as i32;
        let window_phys_h = window_size.height as i32;

        // 工作区 = 去掉任务栏 / Dock 之后的可用区域（全部是物理像素）。
        // 以前用整块显示器尺寸兜底，窗口被夹到底边时会有一截压在任务栏下面。
        let work = monitor.work_area();
        let work_left = work.position.x;
        let work_top = work.position.y;
        let work_right = work.position.x + work.size.width as i32;
        let work_bottom = work.position.y + work.size.height as i32;

        // 光标坐标：`mouse_position` 在 Windows 上走 `GetCursorPos`，返回的就是**物理像素**，
        // 不能再乘 scale。
        //
        // 这里之前写的是 `(x as f64 * scale)`，在 175% 缩放（DPI 168）下把物理坐标又放大了
        // 1.75 倍，位置直接越界，再被边界夹取推到屏幕右下角 —— 表现就是"每次翻译都出现
        // 在屏幕最底下"。
        let phys_x = x;
        let phys_y = y;
        let gap = (CURSOR_GAP * scale) as i32;
        let margin = (EDGE_MARGIN * scale) as i32;

        // 首选位置：光标右下方（custom_x / custom_y 是各平台对锚点的微调，保持原样）
        let mut final_x = phys_x + gap - (custom_x as f64 * scale) as i32;
        let mut final_y = phys_y + gap - (custom_y as f64 * scale) as i32;

        // 下方放不下 → 翻到光标上方。
        //
        // 以前的做法是夹到显示器最底边（`bottom_edge - window_h`），结果窗口高度一变大就
        // 每次都被顶到屏幕最下面，还压在任务栏上——而那片区域其实离你正在看的文字很远。
        // 翻到上方更符合直觉：窗口底边刚好落在光标上面一点点。
        if final_y + window_phys_h > work_bottom - margin {
            final_y = phys_y - gap - window_phys_h;
        }
        // 右侧放不下 → 翻到光标左侧
        if final_x + window_phys_w > work_right - margin {
            final_x = phys_x - gap - window_phys_w;
        }

        // 兑底：不管怎么翻，都完整落在工作区内（两边都放不下时就贴左上角）
        let max_x = (work_right - margin - window_phys_w).max(work_left + margin);
        let max_y = (work_bottom - margin - window_phys_h).max(work_top + margin);
        let final_x = final_x.clamp(work_left + margin, max_x);
        let final_y = final_y.clamp(work_top + margin, max_y);

        let target_pos = PhysicalPosition {
            x: final_x,
            y: final_y,
        };

        debug_log::log(
            "show_position",
            &format!(
                "cursor_phys=({phys_x},{phys_y}) scale={scale} win={window_phys_w}x{window_phys_h} \
                 work=({work_left},{work_top})-({work_right},{work_bottom}) anchor=({custom_x},{custom_y}) \
                 final=({final_x},{final_y})"
            ),
        );

        // 移动位置，等待跨屏合成完成
        let _ = window.set_position(target_pos);

        std::thread::sleep(std::time::Duration::from_millis(10));

        // 再次设置位置
        let _ = window.set_position(target_pos);
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub fn show_input_window(window: WebviewWindow) {
    tauri::async_runtime::spawn(async move {
        window.emit("win-event", json!({"type": "search"})).ok();

        let (custom_x, custom_y) = if cfg!(target_os = "windows") {
            (23, 15)
        } else if cfg!(target_os = "macos") {
            (150, 32)
        } else {
            (23, 15)
        };

        show_main_window(&window, custom_x, custom_y);
    });
}
