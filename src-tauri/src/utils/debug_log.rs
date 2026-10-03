//! 轻量诊断日志。
//!
//! 写在 `%TEMP%\pointra.log`，用于排查"窗口自己消失"这类时序问题：日志里能直接看到
//! 是谁触发的隐藏、当时指针在哪、窗口矩形在哪、判定结果是什么。
//!
//! 只记关键事件（隐藏决策、窗口定位），不做滚动日志，超过上限就清空重来。

use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// 日志文件大小上限，超过就清空重写，避免无限增长
const MAX_LOG_BYTES: u64 = 256 * 1024;

pub fn log_path() -> PathBuf {
    std::env::temp_dir().join("pointra.log")
}

/// 本地时间（UTC+8），形如 `18:42:07.123`
fn local_time() -> String {
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs() + 8 * 3600;
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        (secs / 3600) % 24,
        (secs / 60) % 60,
        secs % 60,
        dur.subsec_millis()
    )
}

pub fn log(tag: &str, msg: &str) {
    let path = log_path();

    if let Ok(meta) = std::fs::metadata(&path) {
        if meta.len() > MAX_LOG_BYTES {
            let _ = std::fs::write(&path, b"");
        }
    }

    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(file, "[{}] {:<16} {}", local_time(), tag, msg);
    }
}
