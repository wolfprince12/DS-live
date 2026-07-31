<div align="center">

<img src="src-tauri/app-icon-source.png" width="120" alt="DSonDT">

# DSonDT

**DeepSeek on Desktop** —— 一个本地运行的 DeepSeek 桌面客户端。

> 本项目与 DeepSeek 官方相互独立，仅使用其公开 API；所有对话与记忆只存你本机，不依赖任何服务端。

---

</div>

## 功能

- **🧠 本地长期记忆（可编辑）**：在「🧠 记忆库」里自己增删改，掌控 AI 对你的长期认知；两种模式下你发出的消息都会自动沉淀。
- **🔑 你自己的 DeepSeek API Key**：系统钥匙串保管；因本地 ad-hoc 签名每次构建指纹会变，自动回退到本地 XOR 加密文件（`apikey.bin`），不落明文、不上传。
- **💬 单窗口双模式**：网页模式（用你自己的 DeepSeek 账号）+ API 模式（自备 Key），同一窗口内切换，共用同一份记忆库。
- **🖥️ 原生 macOS 客户端**：Tauri 2（Rust 后端 + TypeScript 前端），Apple Silicon 原生。
- **📦 多会话、流式输出、深度思考开关、对话导入/导出（JSON）**。

## 下载 / 安装

> 项目无 Apple 开发者账号，CI 出 **ad-hoc 签名**的 `.app`，首次打开会被 Gatekeeper 拦截。

1. 从 [Releases](https://github.com/wolfprince12/DSonDT/releases) 下载最新 `DSonDT.dmg`。
2. 打开 DMG（卷图标即 DSonDT 鲸鱼+大脑 logo），将 `DSonDT.app` 拖入 **应用程序**。
3. 双击 DMG 里的 **`fix.command`**，输入开机密码一键移除隔离限制。
4. 在「应用程序」里**右键 `DSonDT.app` → 打开**（首次需绕过 Gatekeeper）。

如仍提示「无法打开」，请前往 **系统设置 → 隐私与安全性** 点击「仍要打开」，或终端执行 `xattr -cr /Applications/DSonDT.app`。

## 两种模式，共用同一记忆库

| | 🌐 网页模式（默认推荐） | 🔑 API 模式 |
|---|---|---|
| 花费 | 不花钱，用你自己的 DeepSeek 账号 | 按 token 计费，需自备 API Key |
| 界面 | 官网原版，官方改版自动跟上 | 本客户端复刻的 UI |
| 历史 | 天然全在，且与手机端同步 | 只在本地，另有 JSON 导入导出 |
| 记忆注入 | 发送前**前置注入**到消息开头（你自己按回车） | 以 `system` 身份注入，模型遵循度更高 |

网页模式下，点侧边栏「🌐 网页」标签后官方页面内嵌窗口；想让 AI 记得你，在输入框打完字后点右下角 **🧠 注入记忆**（或按 `⌘ M`），程序会把最匹配的几条本地记忆拼到消息开头 —— **发送键仍由你自己按**。

## 它是如何工作的

DSonDT 不打任何服务端、不收集任何数据，全部在本地完成：

- **记忆存储**：每条记忆以文本存入本地 SQLite；有 API Key 时额外生成 embedding 向量，检索走「向量余弦 + 字符二元组」双路匹配。
- **记忆检索**：新对话时取最相关 Top-K（相似度 ≥ 0.25）注入上下文；无 Key 时自动退化为字符二元组关键词检索，零成本可用。
- **记忆沉淀**：两种模式下你发出的消息都会自动写入记忆库，也可在「🧠 记忆库」面板手动编辑。
- **密钥保护**：API Key 优先写入 macOS 钥匙串；钥匙串不可用时回退到本地 XOR 加密文件，权限 `600`，绝不落明文、绝不上传。

## 自行构建

前置：Rust 工具链 + Xcode Command Line Tools + Node.js 20+。

```bash
cd src && npm install        # 前端依赖
npm run tauri dev            # 开发模式（热更新）
npm run tauri build          # 打包：.app + .dmg
```

首次启动在「设置」中填入 DeepSeek API Key。

## 关于作者

**Mr大狼** —— 二十年影视传媒老兵，做过音乐节、纪录片、综艺导演，作品上过央视春晚、北影节音乐节。八年前独立运营「大狼导演工作室」，近年转型用 AI 把跨界底子焊成产品：爻知云 微信服务号、DealV智能合同平台、鼠须管输入法图形控制台等

DSonDT 最初是给自己做的工具——让 AI 真的「记得」我。后来觉得「想让 AI 记得自己」这件事大家都需要，就开源了出来。

云珩（北京）文化传媒有限公司 出品。

## 赞助 · 请杯咖啡

如果这个项目帮到了你，欢迎扫微信二维码请作者一杯咖啡 ☕ —— 所有赞助都会变成 DeepSeek API token，继续测试和迭代。

<img src="assets/payment-qr.png" width="240" alt="微信支付二维码">

也可以留个 Star ⭐，这是对独立开发者最大的鼓励。

## 许可

[MIT](./LICENSE)

---

# DSonDT (English)

**DeepSeek on Desktop** — a locally-run DeepSeek desktop client.

> This project is independent of DeepSeek; it only uses their public API, and all conversations and memories stay on your machine — no server required.

## Features

- **🧠 Editable local long-term memory**: add/edit/delete in the "🧠 Memory" panel; messages you send in either mode are auto-captured.
- **🔑 Your own DeepSeek API Key**: stored in the macOS Keychain, with automatic fallback to a local XOR-encrypted file (`apikey.bin`) — never plaintext, never uploaded.
- **💬 Single-window dual mode**: Web mode (your DeepSeek account) + API mode (your own Key), switched in-app and sharing one memory store.
- **🖥️ Native macOS client**: Tauri 2 (Rust backend + TypeScript frontend), Apple Silicon native.
- **📦 Multi-session, streaming, thinking toggle, conversation import/export (JSON).**

## Install

> No Apple Developer account — CI produces an **ad-hoc signed** `.app`, so the first launch is blocked by Gatekeeper.

1. Download the latest `DSonDT.dmg` from [Releases](https://github.com/wolfprince12/DSonDT/releases).
2. Open the DMG and drag `DSonDT.app` into **Applications**.
3. Double-click **`fix.command`** inside the DMG and enter your password to clear the quarantine.
4. In **Applications**, **right-click `DSonDT.app` → Open** (needed once to bypass Gatekeeper).

If it still says "cannot be opened", go to **System Settings → Privacy & Security** and click "Open Anyway", or run `xattr -cr /Applications/DSonDT.app` in Terminal.

## How it works

DSonDT runs no server and collects no data — everything happens locally:

- **Memory storage**: each memory is stored as text in local SQLite; with an API Key an embedding vector is also generated, retrieved via "cosine + character bigram" dual-path matching.
- **Memory retrieval**: on a new conversation the top-K relevant memories (similarity ≥ 0.25) are injected into context; without a Key it degrades to character-bigram keyword search, free to use.
- **Key protection**: the API Key goes to the macOS Keychain first, falling back to a local XOR-encrypted file (`600` perms) — never plaintext, never uploaded.

## About the Author

**Mr. Dawolf** — 20+ years in film, TV and live production; runs "Big Wolf Director Studio" independently; now rebuilding cross-domain experience into AI products: 爻知云 WeChat Official Account, DealV smart contract platform, Squirrel Panel (input-method GUI), and more.

DSonDT started as a personal tool to make AI truly "remember" me. Released open-source because everyone needs an AI that remembers them.

Produced by 云珩（北京）文化传媒有限公司.

## Sponsor · Buy Me a Coffee

If this project helped you, feel free to buy me a coffee via WeChat Pay ☕

<img src="assets/payment-qr.png" width="240" alt="WeChat Pay QR Code">

## License

[MIT](./LICENSE)
