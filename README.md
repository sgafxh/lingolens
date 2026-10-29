# LingoLens · 译镜

<img src="public/lingolens.svg" width="112" height="112" alt="LingoLens icon" />

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust + Tauri](https://img.shields.io/badge/Desktop-Rust%20%2B%20Tauri-24b7bf)](src-tauri)

**光标指到哪里，就从哪里开始翻译。**

LingoLens 是一款桌面翻译工具，支持光标取词、划词翻译和手动输入。中文按词识别，中英混排自动判断翻译方向，长文以 Markdown 展示，适合阅读技术文档、论文和外文资料。

A desktop translator for Chinese-English reading, with cursor OCR, word segmentation, and Markdown support.

## 功能

- **光标取词** — 按下 F2，识别光标附近的文字。中文结合词频分词确定词边界，英文按单词取词。
- **划词与输入翻译** — 翻译选中的文本，或在搜索窗口直接输入内容。
- **中英混排** — 自动判断翻译方向，也可指定中文或英文为目标语言。
- **Markdown 阅读** — 展示标题、列表和代码，保留段落与换行，支持滚动阅读长译文。
- **随手调整窗口** — 拖拽缩放、记忆尺寸、窗口置顶，方便边读边查。
- **模型配置** — 自定义翻译服务地址、模型和 API Key。

## 快速开始

### Windows 安装

从 [GitHub Releases](https://github.com/sgafxh/lingolens/releases/latest) 下载 **LingoLens_1.0.1_x64-setup.exe**，运行安装后，在首次打开的设置窗口中配置翻译服务。

适用于 Windows 10/11 x64，需要 WebView2；中文 OCR 需要系统安装中文语言包。每个版本附带 SHA256 校验文件。

### 从源码运行

准备 Node.js 22.12+、pnpm、Rust stable 和 [Tauri 平台依赖](https://v2.tauri.app/start/prerequisites/)。Windows 开发环境还需要 MSVC C++ 构建工具。

    git clone https://github.com/sgafxh/lingolens.git
    cd lingolens
    pnpm install --frozen-lockfile
    pnpm tauri dev

前端构建：运行 pnpm build。桌面打包：运行 pnpm tauri build。

## 配置翻译服务

首次启动会打开设置窗口。也可通过托盘菜单进入 **设置界面 → 翻译服务**，填写服务地址、模型名称和 API Key，选择目标语言后点击 **保存翻译服务**。

API Key 默认隐藏；更换地址、模型或目标语言时，Key 留空即可保留已保存的密钥。保存后的设置用于下一次翻译。

目标语言可选自动、中文或英文。光标取到中文时译为英文，划词和输入翻译遵循目标语言设置。

## 快捷键

| 操作 | Windows | macOS |
| --- | --- | --- |
| 光标取词 | F2 | F2 |
| 窗口置顶 | F1 | F1 |
| 划词翻译 | Alt+1 | Cmd+1 |
| 输入翻译 | Alt+2 | Cmd+2 |

所有快捷键均可在设置中调整。

## 数据与隐私

截图和 OCR 在本机完成，待翻译文本发送到你配置的模型服务。发音功能通过有道语音接口获取音频。API Key 保存在本机配置文件中，服务费用按所选提供商的规则计算。

诊断日志默认关闭。排查问题时可设置 LINGOLENS_DEBUG=1，日志写入临时目录的 lingolens.log，可能包含原文片段；分享前请清理敏感内容。

## 开发

项目使用 **Rust + Tauri 2 + React + TypeScript**。Windows 构建与测试记录见 [验证说明](docs/VALIDATION.md)，macOS 验证进度见 [开发记录](docs/CHANGES.md)。

运行 Rust 单元测试：

    cargo test --manifest-path src-tauri/Cargo.toml --lib

独立运行 OCR 与分词测试：

    rustc --edition 2021 --test scripts/core_tests.rs -o core-tests
    ./core-tests

Windows 下可将输出文件命名为 core-tests.exe。提交问题和改进请参阅 [贡献指南](CONTRIBUTING.md)。

## 许可与致谢

LingoLens 由 [Xuehan Feng (@sgafxh)](https://github.com/sgafxh) 维护，基于 [Pointra](https://github.com/SYJun404/Pointra-Pub) 开发，采用 [MIT 许可证](LICENSE)。感谢 SYJun、[SnapTraTranslator](https://github.com/yelog/SnapTraTranslator) 和 [jieba](https://github.com/fxsjy/jieba) 的工作。第三方资源说明见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。
