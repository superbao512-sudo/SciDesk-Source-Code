# SciDesk 0.3.2 · 源码开发指南

SciDesk 是面向 Windows 的本地科研工作台。这个工程包含桌面应用的 React/TypeScript 前端、Rust/Tauri 后端、SQLite 迁移、测试，以及构建公开 Windows 安装包所需的产品资源。使用软件的操作步骤见[中文使用指南](docs/USER-GUIDE.zh-CN.md)。

## 新电脑需要安装什么

| 配置 | 用途与安装选择 |
| --- | --- |
| Windows 10/11 x64 | 桌面窗口、系统回收站、截图、文件拖放和安装器面向 Windows。将工程解压至当前用户可写的本地目录，例如 `D:\Dev\SciDesk-0.3.2`。 |
| Node.js 24.x 与 npm | 开发服务和测试使用内置的 `node:sqlite`。前端依赖版本保存在 `package-lock.json`。 |
| Rust stable 与 Cargo | Windows 新环境可通过 rustup 安装 `stable-x86_64-pc-windows-msvc`；Rust 依赖由 `src-tauri/Cargo.lock` 锁定。 |
| Visual Studio Build Tools 与 Windows SDK | 采用 MSVC 时，选择“使用 C++ 的桌面开发”、x64 MSVC 编译工具、Windows SDK 和 CMake 工具。建议从 **Developer PowerShell for Visual Studio** 打开工程，让原生依赖找到编译器。 |
| Microsoft Edge WebView2 Runtime | Tauri 桌面窗口用它渲染界面。 |
| 网络 | 首次安装 npm/Cargo 依赖及首次构建 NSIS 安装包需要联网。GitHub 更新查询、期刊刷新及联网翻译在使用对应功能时访问网络。 |

Python 3 仅在运行 Python 辅助脚本时需要。重新从 Excel 生成学习模块的测试目录时，还需 `openpyxl`。正常开发启动使用 Node、Cargo 和本机编译工具即可。

安装入口：[Node.js](https://nodejs.org/en/download/) · [Rust](https://rustup.rs/) · [Tauri Windows 环境说明](https://v2.tauri.app/start/prerequisites/#windows) · [WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/)

## 首次启动

解压后，在含有 `package.json` 的目录打开 PowerShell。检查环境并还原依赖：

```powershell
node -v
npm.cmd -v
rustc -V
cargo -V
cmake --version
npm.cmd ci
npm.cmd run tauri:dev
```

桌面开发命令会启动 Vite、编译本地桥接程序并打开 SciDesk 窗口。只调试网页界面时，使用 `npm.cmd run dev`，再打开 `http://127.0.0.1:1421/`。两种方式共用 1421 端口，选择其中一种即可。桌面文件选择器、截图和原生窗口等功能请在桌面模式下验证。

## 从哪里开始改

| 目录 | 内容 |
| --- | --- |
| `src/features/` | 项目、期刊、学习、想法、手册和工具等功能页面。工具卡注册在 `tools/builtinTools.ts`。 |
| `src/layouts/` | 主窗口布局、侧栏和“检查更新”入口。 |
| `src-tauri/src/` | 文件操作、各业务服务、数据存储与平台集成；`updates.rs` 定义 GitHub 正式版查询地址。 |
| `src-tauri/*migrations/` | SQLite 结构演进。改表时保留既有用户数据，优先使用增量迁移。 |
| `src-tauri/resources/awbf/` | 随程序提供的 AWBF Windows 可执行文件与运行库。此包提供 SciDesk 的启动集成源码；AWBF 自身为单独构建的工具。更换其程序时需同步核对启动代码中的可执行文件摘要。 |
| `scripts/`、`tests/` | 开发桥接、资源处理、构建辅助脚本及 Node 测试。Rust 测试与对应模块放在一起。 |

例如，修改“学习”页导入提示时，从 `src/features/learning/BookmarkImport.tsx` 入手；改变分类持久化行为时，同时检查 `src-tauri/src/learning.rs`。个人项目、真实书签、数据库和编译缓存不随包提供。旧书签导入测试使用生成的测试工作簿和配套目录，数据不会作为新用户的默认书签。

## 测试与构建

```powershell
npm.cmd test
npm.cmd run typecheck
npm.cmd run build
npm.cmd run tauri:check
npm.cmd run rust:test
```

`npm.cmd run build` 生成 `dist/` 前端资源。需要可分发的 Windows 安装包时，在工程根目录运行：

```powershell
$env:CARGO_TARGET_DIR = Join-Path $PWD 'development\tauri-target'
npm.cmd run tauri -- build --config src-tauri/tauri.public.conf.json --bundles nsis --ci
Get-ChildItem "$env:CARGO_TARGET_DIR\release\bundle\nsis" -Filter *.exe
```

公开发行使用 `tauri.public.conf.json`。项目中另有显式收集个人快照的 `package:personal` 脚本，仅适合自用。发布新版本时同步更新 `package.json`、`package-lock.json`、`src-tauri/Cargo.toml`、`src-tauri/Cargo.lock` 与 Tauri 配置中的应用版本，生成安装包的 SHA-256 校验文件，再用对应的 `v主版本.次版本.修订号` 标签创建正式 Release。仓库地址变更时也要更新 `updates.rs`。

## 数据位置与排错

- 开发桌面数据在工程的 `runtime/desktop/`，浏览器开发数据在 `runtime/browser/`。安装版使用程序旁的 `SciDeskData/`。验证升级时应使用测试副本。
- 提示缺少 `node:sqlite` 时，先用 `node -v` 检查当前终端实际加载的 Node。提示缺少链接器、SDK 或 CMake 时，检查 C++ 桌面开发组件并重新打开 Developer PowerShell。
- 依赖下载失败时，检查 npm registry、Cargo 和 GitHub 的网络连接。遇到 1421 端口占用，先关闭同一工程中已启动的开发服务。
- 桌面窗口无法加载时，检查 WebView2 Runtime。运行打包命令前，应保留完整的 `src-tauri/resources/awbf` 目录。
