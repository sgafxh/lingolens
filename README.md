# LingoLens · 译镜

<img src="public/lingolens.svg" width="112" height="112" alt="LingoLens icon" />

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust + Tauri](https://img.shields.io/badge/Desktop-Rust%20%2B%20Tauri-24b7bf)](src-tauri)
[![Validation](https://img.shields.io/badge/Windows_unit_tests-27_passed-2f855a)](docs/VALIDATION.md)

光标取词、划词翻译与 Markdown 阅读，面向中英文混排的桌面工作流。

LingoLens is a Tauri desktop translator with Chinese word segmentation, cursor-based OCR, and paragraph-aware Markdown translation. Maintained by [Xuehan Feng (@sgafxh)](https://github.com/sgafxh).

基于 [SYJun404/Pointra-Pub](https://github.com/SYJun404/Pointra-Pub) 持续改进。Pointra 原始作者为 SYJun，其 README 说明项目基于 [yelog/SnapTraTranslator](https://github.com/yelog/SnapTraTranslator)。本仓库保留上游提交历史；新增工作见 [改进记录](docs/CHANGES.md)。

## 为什么做这个项目

中文技术阅读常会碰到三个问题：光标 OCR 只能取到单字、混排内容容易选错翻译方向、长译文丢失段落和代码结构。LingoLens 把中文词边界、语言判断和 Markdown 展示放在同一条桌面翻译流程中，方便阅读英文文档、论文和含代码的技术内容。

## 项目状态

当前提供源码，优先面向 Windows 开发与验证。继承了 macOS 实现，但本分支的 macOS 构建与 OCR 行为仍待验证。安装包和自动更新通道尚未发布。

## 本分支的改进

- **中文光标取词**：将 OCR 字框与中文词边界结合，使用由 jieba 词典生成的词频表，减少相邻词误合并。
- **中英混排翻译**：按汉字和英文单词判断语言；支持自动、中文、英文目标语言，并对疑似未翻译的结果重试一次。
- **完整段落与 Markdown**：保留换行，展示标题、列表、代码等 Markdown 内容，长译文可滚动阅读。
- **窗口交互**：拖拽调整尺寸并记忆大小，处理缩放期间自动隐藏与页脚按钮被遮挡的问题。
- **可配置模型后端**：通过 Chat Completions 形式的接口请求翻译，本地配置服务地址、模型和 API Key。当前请求参数针对法衡-H1，其他服务需验证兼容性。

## 从源码运行

需要 Node.js 22.12+、pnpm、Rust stable，以及 [Tauri 平台前置依赖](https://v2.tauri.app/start/prerequisites/)。Windows 需 MSVC C++ 构建工具与 WebView2，中文 OCR 需相应的 Windows 语言包。

    git clone https://github.com/sgafxh/lingolens.git
    cd lingolens
    pnpm install --frozen-lockfile
    pnpm tauri dev

构建前端：运行 pnpm build。构建桌面安装包：运行 pnpm tauri build。

## 配置翻译服务

先运行一次应用，再退出应用并编辑配置文件。Windows 路径：

    %APPDATA%\io.github.sgafxh.lingolens\config.json

在已有的 config 对象内修改以下字段，保留其余设置：

    {
      "llm_base_url": "https://api.faheng.chat/v1",
      "llm_model": "法衡-H1",
      "llm_api_key": "YOUR_API_KEY",
      "llm_target_lang": "auto"
    }

llm_target_lang 可选 auto、zh、en。光标取到中文时固定译为英文；划词和输入翻译遵循目标语言配置。接口会发送 reasoning_effort: "none"，服务端须支持此参数。翻译服务费用由所选服务商收取。

新的应用标识使用独立配置目录；已有 Pointra 用户可手动复制需要的配置字段。API Key 存储在本机配置文件中，勿上传配置文件或日志。

## 快捷键与数据流

| 操作 | 默认快捷键 |
| --- | --- |
| 光标取词 | F2 |
| 窗口置顶 | F1 |
| 划词翻译 | Windows Alt+1 / macOS Cmd+1 |
| 输入翻译 | Windows Alt+2 / macOS Cmd+2 |

快捷键可在设置中修改。截图和 OCR 在本机执行；识别后的文本发送给配置的翻译服务。发音功能会向有道语音接口发送待朗读的单词。诊断日志默认关闭，设置 LINGOLENS_DEBUG=1 后写入临时目录的 lingolens.log，其中可能含原文片段。

## 开发与贡献

见 [CONTRIBUTING.md](CONTRIBUTING.md)。OCR 与分词逻辑的快速测试：

    rustc --edition 2021 --test scripts/core_tests.rs -o core-tests
    ./core-tests

Windows 可将输出名改为 core-tests.exe。完整 Rust 单元测试：

    cargo test --manifest-path src-tauri/Cargo.toml --lib

## 许可证与致谢

代码按 [MIT](LICENSE) 发布，保留 SYJun 的原始版权声明。词典与继承资源的来源见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。感谢 Pointra、SnapTraTranslator、jieba 及相关依赖的作者。
