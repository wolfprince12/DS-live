<div align="center">

<img src="src-tauri/app-icon-source.png" width="120" alt="DSonDT">

# DSonDT

**DeepSeek on Desktop** —— 一个本地运行的 DeepSeek 桌面客户端。

> 本项目与 DeepSeek 官方相互独立，仅使用其公开 API；所有对话与记忆只存你本机，不依赖任何服务端。

![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Windows-000000?logo=apple&logoColor=white)
![Tauri](https://img.shields.io/badge/Tauri-2-24C8DB?logo=tauri&logoColor=white)

**macOS 版与 Windows 版均已上线，功能完全一致**：本地长期记忆、网页模式 + API 模式单窗口切换、你自己的 DeepSeek API Key 本地保管。

---

</div>

## 功能

- **本地长期记忆（可编辑）**：在「记忆库」里自己增删改，掌控 AI 对你的长期认知；两种模式下你发出的消息都会自动沉淀。
- **你自己的 DeepSeek API Key**：macOS 存入系统钥匙串（Keychain），Windows 存入凭据管理器（Credential Manager），不可用时回退到本地 XOR 加密文件，不落明文、不上传。
- **单窗口双模式**：网页模式（用你自己的 DeepSeek 账号）+ API 模式（自备 Key），同一窗口内切换，共用同一份记忆库。
- **原生客户端**：基于 Tauri 2（Rust 后端 + TypeScript 前端）；macOS 用 WKWebView，Windows 用 WebView2 原生渲染。
- **多会话、流式输出、深度思考开关、对话导入/导出（JSON）。**

## 下载 / 安装

### macOS

1. 从 [Releases](https://github.com/wolfprince12/DSonDT/releases) 下载 `DSonDT-{version}-aarch64.dmg`（Apple Silicon）。
2. 打开 DMG，把 `DSonDT.app` 拖到「应用程序」。
3. **首次打开会被 Gatekeeper 拦下**（本应用使用 ad-hoc 签名、未购买 Apple 开发者公证）：
   - 方式一：在「应用程序」里**右键 → 打开**，在弹窗里点「打开」；
   - 方式二：双击 DMG 内的 `fix.command`（首次需允许「终端」运行），一键清除隔离属性。
4. 没有 Apple 开发者账号，所以这是预期行为，并非软件有问题。

### Windows

> 提供两种形态：**绿色便携版**（解压即用，无需安装）与 **安装包**（.msi / .nsis）。

1. **绿色便携版（推荐，免安装）**：从 Releases 下载 `DSonDT-x.x.x-windows-portable.zip`，解压后直接双击 `DSonDT.exe` 即可运行，可放任意目录或 U 盘。
2. **安装包**：下载 `DSonDT_x64_en-US.msi`（或 .exe 安装向导），按提示安装，从开始菜单 / 桌面快捷方式启动。
3. **环境要求**：Windows 10 / 11（64 位），并已安装 Microsoft Edge **WebView2 Runtime**（Win11 自带；Win10 未装请到微软官网下载）。

## 两种模式，共用同一记忆库

| | 网页模式（默认推荐） | API 模式 |
|---|---|---|
| 花费 | 不花钱，用你自己的 DeepSeek 账号 | 按 token 计费，需自备 API Key |
| 界面 | 官网原版，官方改版自动跟上 | 本客户端复刻的 UI |
| 历史 | 天然全在，且与手机端同步 | 只在本地，另有 JSON 导入导出 |
| 记忆注入 | 发送前**前置注入**到消息开头（你自己按回车） | 以 system 身份注入，模型遵循度更高 |

网页模式下，点侧边栏「网页」标签后官方页面整页打开；想让 AI 记得你，在输入框打完字后点右下角 **注入记忆**（或按 Ctrl/Cmd + M），程序会把最匹配的几条本地记忆拼到消息开头 —— **发送键仍由你自己按**。

## 它是如何工作的

DSonDT 不打任何服务端、不收集任何数据，全部在本地完成：

- **记忆存储**：每条记忆以文本存入本地 SQLite；有 API Key 时额外生成 embedding 向量，检索走「向量余弦 + 字符二元组」双路匹配。
- **记忆检索**：新对话时取最相关 Top-K（相似度 >= 0.25）注入上下文；无 Key 时自动退化为字符二元组关键词检索，零成本可用。
- **记忆沉淀**：两种模式下你发出的消息都会自动写入记忆库，也可在「记忆库」面板手动编辑。
- **密钥保护**：API Key 在 macOS 写入系统钥匙串、在 Windows 写入凭据管理器；不可用时回退到本地 XOR 加密文件，权限受限，绝不落明文、绝不上传。

## 自行构建

### macOS

前置：macOS 12+ + Rust + Node.js 22+。

    cd src && npm install        # 前端依赖
    npm run tauri dev            # 开发模式（热更新）
    npm run tauri build          # 打包：生成 .app / .dmg

### Windows

前置：Windows 10/11 + Rust（MSVC 工具链）+ Node.js 22+ + WebView2 Runtime。

    cd src && npm install        # 前端依赖
    npm run tauri dev            # 开发模式（热更新）
    npm run tauri build          # 打包：生成 .msi / .nsis 安装包

> 若只需**绿色便携版**：用 Tauri CLI 直接出 exe，再把 `src-tauri/target/release/DSonDT.exe` 压缩即可：
>
>     node ./src/node_modules/@tauri-apps/cli/tauri.js build --no-bundle

首次启动在「设置」中填入 DeepSeek API Key。

## 关于作者

**Mr大狼** —— 二十年影视传媒老兵，做过音乐节、纪录片、综艺导演，作品上过央视春晚、北影节音乐节。八年前独立运营「大狼导演工作室」，近年转型用 AI 把跨界底子焊成产品：爻知云 微信服务号、DealV智能合同平台、鼠须管输入法图形控制台等。

DSonDT 最初是给自己做的工具——让 AI 真的「记得」我。后来觉得「想让 AI 记得自己」这件事大家都需要，就开源了出来。

## 赞助 · 请杯咖啡

如果这个项目帮到了你，欢迎扫微信二维码请作者一杯咖啡 —— 所有赞助都会变成 DeepSeek API token，继续测试和迭代。

<img src="assets/payment-qr.png" width="240" alt="微信支付二维码">

也可以留个 Star，这是对独立开发者最大的鼓励。

## 许可

[MIT](./LICENSE)

---

# DSonDT — English

**DeepSeek on Desktop** — a locally-run DeepSeek desktop client.

> This project is independent of DeepSeek; it only uses their public API, and all conversations and memories stay on your machine — no server required.

**Both macOS and Windows builds are available**, with identical features.

## Features

- **Editable local long-term memory**: add/edit/delete in the Memory panel; messages you send in either mode are auto-captured.
- **Your own DeepSeek API Key**: stored in the macOS Keychain / Windows Credential Manager first, with automatic fallback to a local XOR-encrypted file — never plaintext, never uploaded.
- **Single-window dual mode**: Web mode (your DeepSeek account) + API mode (your own Key), switched in-app and sharing one memory store.
- **Native client**: Tauri 2 (Rust backend + TypeScript frontend); WKWebView on macOS, WebView2 on Windows.
- **Multi-session, streaming, thinking toggle, conversation import/export (JSON).**

## Install

- **macOS**: download `DSonDT-{version}-aarch64.dmg` from Releases, drag `DSonDT.app` to Applications. First launch is blocked by Gatekeeper (ad-hoc signed, not notarized) — right-click → Open, or run `fix.command` from the DMG to clear the quarantine attribute.
- **Windows**: download `DSonDT-x.x.x-windows-portable.zip` (portable) or the `.msi` / `.exe` installer. Requires Windows 10/11 (64-bit) with the Microsoft Edge WebView2 Runtime.

## Build from source

- **macOS**: `cd src && npm install` → `npm run tauri dev` (dev) / `npm run tauri build` (DMG).
- **Windows**: `cd src && npm install` → `npm run tauri dev` / `npm run tauri build` (.msi / .nsis).

## About the Author

**Mr. Dawolf** — 20+ years in film, TV and live production; runs Big Wolf Director Studio independently; now rebuilding cross-domain experience into AI products: YaoZhiYun WeChat Official Account, DealV smart contract platform, Squirrel Panel (input-method GUI), and more.

DSonDT started as a personal tool to make AI truly remember me. Released open-source because everyone needs an AI that remembers them.

## License

[MIT](./LICENSE)
