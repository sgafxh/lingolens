//! 窗口尺寸落盘（防抖）。
//!
//! 主窗口可以拖右下角缩放，尺寸需要记住。保存时机不能只靠前端 `pointerup`：
//! 拖拽过程中指针可能压出窗口而收不到 `pointerup`，前端手里的尺寸也可能因为
//! `requestAnimationFrame` 合帧而偏旧（之前就是这里把尺寸写漏了）。
//!
//! 所以这里直接监听窗口的 `Resized` 事件，防抖后读取**真实**尺寸写回配置；
//! 前端那条命令只是顺带加速，两条路共用同一个写入函数。

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Manager, Runtime};

/// 防抖窗口：最后一次尺寸变化后等这么久再落盘
const DEBOUNCE: Duration = Duration::from_millis(600);

static SAVE_PENDING: AtomicBool = AtomicBool::new(false);
/// 最近一次已落盘的尺寸（宽高各打包成 32 位），用来避免重复写文件
static LAST_SAVED: AtomicU64 = AtomicU64::new(0);

fn pack(width: f64, height: f64) -> u64 {
    let w = (width.round().max(0.0) as u64) & 0xffff_ffff;
    let h = (height.round().max(0.0) as u64) & 0xffff_ffff;
    (w << 32) | h
}

/// 标记某个尺寸已经落盘（前端保存成功后调用）
pub fn mark_saved(width: f64, height: f64) {
    LAST_SAVED.store(pack(width, height), Ordering::SeqCst);
}

/// 窗口尺寸变化后调用：防抖后落盘
pub fn schedule_save<R: Runtime>(app: AppHandle<R>) {
    if SAVE_PENDING.swap(true, Ordering::SeqCst) {
        return;
    }

    std::thread::spawn(move || {
        std::thread::sleep(DEBOUNCE);
        SAVE_PENDING.store(false, Ordering::SeqCst);

        let Some(window) = app.get_webview_window("main") else {
            return;
        };

        let Ok(size) = window.inner_size() else {
            return;
        };
        let Ok(scale) = window.scale_factor() else {
            return;
        };
        if scale <= 0.0 {
            return;
        }

        // 配置里存的是逻辑像素（与 tauri.conf.json / 前端一致）
        let width = size.width as f64 / scale;
        let height = size.height as f64 / scale;

        if LAST_SAVED.load(Ordering::SeqCst) == pack(width, height) {
            return;
        }

        if crate::commands::window::persist_window_size(&app, width, height).unwrap_or(false) {
            mark_saved(width, height);
        }
    });
}
