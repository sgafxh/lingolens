//! 版本更新检查
//!
//! 上游仓库里这个文件是私有的（`.gitignore` 标记为 Private files）。这里给出一个
//! **故意不做任何检查** 的最小实现，原因：
//!
//! 本构建把翻译后端换成了法衡-H1，并且带了若干行为改动，与上游官方发布的 Pointra
//! 不是同一个东西。如果照旧去比对官方版本号，界面上会冒出「版本更新」按钮，用户
//! 一点就被带到官方 release 页；装上官方版会把自研后端和这些修复一起冲掉。
//!
//! 所以这里恒返回 `None`（= 没有更新）。将来若要给自己的构建做更新，
//! 在这里换成请求你自己的版本清单即可，接口形状保持不变。

use serde::{Deserialize, Serialize};

/// 更新信息：前端只用到 `url`（点按钮后打开这个地址）
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UpdateCheckResponse {
    pub url: String,
}

/// 返回 `Some((更新信息, 是否有更新))`
///
/// 恒为 `None`：本构建不提示更新。见文件头注释。
pub async fn check_for_update(_version: &str) -> Option<(UpdateCheckResponse, bool)> {
    None
}
