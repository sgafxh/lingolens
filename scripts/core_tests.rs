//! Standalone harness for the existing OCR and segmentation unit tests.
#[path = "../src-tauri/src/utils/zh_segment.rs"]
pub mod zh_segment;
mod utils {
    pub use crate::zh_segment;
}
#[path = "../src-tauri/src/utils/ocr_common.rs"]
mod ocr_common;
