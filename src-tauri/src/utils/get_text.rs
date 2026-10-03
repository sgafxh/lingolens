#[cfg(target_os = "macos")]
use super::selection_mac::get_text;
#[cfg(target_os = "macos")]
use crate::utils::capture_mac::capture_around_cursor;
#[cfg(target_os = "macos")]
use crate::utils::ocr_mac::{recognize_words, select_sentence, select_word};

#[cfg(target_os = "windows")]
use super::selection_win::get_text;
#[cfg(target_os = "windows")]
use crate::utils::capture_win::capture_around_cursor;
#[cfg(target_os = "windows")]
use crate::utils::ocr_win::{recognize_words, select_sentence, select_word};

use crate::utils::debug_log;
use crate::utils::show_window::show_main_window;
use crate::AppState;

use tauri::{AppHandle, Emitter, Manager, Runtime, State, WebviewWindow};

/// 取词 / 截图前先把自己藏起来。
///
/// 窗口是置顶的，如果它正好盖住光标或选区所在的区域：
/// 1. 屏幕截图会把 Pointra 自己的界面交给 OCR，翻出来的是自己的译文
/// 2. 窗口占着焦点，目标应用的选区 / UIA 取不到文本
/// 隐藏之后目标应用会重新拿到焦点，选区仍然在。
fn hide_self<R: Runtime>(window: &WebviewWindow<R>) {
    if window.is_visible().unwrap_or(false) {
        debug_log::log("hide_self", "取词/划词前先收起窗口");
        let _ = window.hide();
        std::thread::sleep(std::time::Duration::from_millis(120));
    }
}

pub fn get_data_from_selected_text(window: WebviewWindow, app: &AppHandle) {
    let win_clone = window.clone();
    let app_clone = app.clone();

    tauri::async_runtime::spawn(async move {
        // 上一次翻译的窗口可能还开着，先收起来再取词
        hide_self(&window);

        #[cfg(target_os = "macos")]
        let text_opt = get_text(&window).filter(|(text, _, _)| !text.trim().is_empty());
        #[cfg(target_os = "windows")]
        let text_opt = get_text().filter(|(text, _, _)| !text.trim().is_empty());

        if let Some((text, custom_x, custom_y)) = text_opt {
            window.emit("from-cursor", text).ok();
            show_main_window(&window, custom_x, custom_y);
        } else {
            let app_state = app_clone.state::<AppState>();
            get_sentence_under_cursor(app_state, win_clone);
        }
    });
}

pub fn get_word_under_cursor(app_state: State<'_, AppState>, window: WebviewWindow) {
    // 窗口如果开着会盖住光标下的文字，先收起来再截图
    hide_self(&window);

    #[cfg(target_os = "macos")]
    {
        let capture_res = capture_around_cursor(200, 40);
        let Ok(img) = capture_res else { return };
        let words_res = recognize_words(&img, &app_state.ocr_state);
        let Ok(words) = words_res else { return };
        if let Some(word) = select_word(&words) {
            if !word.is_empty() {
                show_main_window(&window, 0, 0);
                window.emit("from-cursor", word).ok();
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        let capture_res = capture_around_cursor(200, 40);
        let Ok(img) = capture_res else { return };
        let words_res = recognize_words(&img, &app_state.ocr_state);
        let Ok(words) = words_res else { return };
        if let Some(word) = select_word(&words) {
            if !word.is_empty() {
                show_main_window(&window, 0, 0);
                window.emit("from-cursor", word).ok();
            }
        }
    }
}

pub fn get_sentence_under_cursor(app_state: State<'_, AppState>, window: WebviewWindow) {
    // 同上：截图前先隐藏自身，避免 OCR 识别到自己的界面
    hide_self(&window);

    #[cfg(target_os = "macos")]
    {
        let capture_res = capture_around_cursor(800, 50);
        let Ok(img) = capture_res else { return };
        let words_res = recognize_words(&img, &app_state.ocr_state);
        let Ok(words) = words_res else { return };
        if let Some(sentence) = select_sentence(&words) {
            if !sentence.is_empty() {
                show_main_window(&window, 16, 18);
                window.emit("from-cursor", sentence).ok();
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        let capture_res = capture_around_cursor(800, 50);
        let Ok(img) = capture_res else { return };
        let words_res = recognize_words(&img, &app_state.ocr_state);
        let Ok(words) = words_res else { return };
        if let Some(sentence) = select_sentence(&words) {
            if !sentence.is_empty() {
                show_main_window(&window, 23, 15);
                window.emit("from-cursor", sentence).ok();
            }
        }
    }
}
