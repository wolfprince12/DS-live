# DSonMac

本地运行的 **DeepSeek 桌面客户端**，弥补 DeepSeek 网页版「没有长期记忆」和「没有桌面客户端」两个短板。自己持有 API Key 调用 DeepSeek API，全部对话与长期记忆存储在本地，**Mac（仅 Apple Silicon）+ Windows 双端**，开源免费。

## 特性

- 🖥️ 原生桌面客户端（Mac / Windows），UI 完全复刻 DeepSeek 网页端
- 🧠 **长期记忆**：用 DeepSeek embedding 把历史对话转向量存本地，新对话自动检索相关片段注入上下文
- 🔑 API Key 存入系统钥匙串（macOS 钥匙串 / Windows 凭据管理器），不落明文、不上传
- 💬 多会话管理、流式输出、深度思考开关（`thinking`）
- 📦 对话导入 / 导出（JSON）
- 🆓 开源免费（MIT），低维护（一套 Tauri 代码出双端，CI 自动构建）

## 技术栈

- [Tauri 2](https://v2.tauri.app/)（Rust 后端 + Web 前端，一套代码出双端）
- Rust：`reqwest`（流式调用 DeepSeek API）、`rusqlite`（本地存储）、`keyring`（密钥保管）
- 前端：原生 TypeScript + Vite
- DeepSeek API：`deepseek-v4-flash` / `deepseek-v4-pro`，embedding 模型 `deepseek-embedding`

## 开发构建

> 前置：Rust 工具链 + Xcode Command Line Tools（Mac）/ 无需 Windows 机器（CI 自动构建）

```bash
# 1. 安装前端依赖
cd src
npm install

# 2. 开发模式（热更新）
npm run tauri dev

# 3. 打包（Mac 出 .app/.dmg，Windows 出 .msi/.exe）
npm run tauri build
```

首次启动在「设置」中填入 DeepSeek API Key（https://platform.deepseek.com/api_keys）。

## 发布（双端 CI）

打 tag 即可触发 GitHub Actions，自动构建 Mac(arm64) 与 Windows(x64) 并发布 Release：

```bash
git tag v0.1.0
git push origin v0.1.0
```

- **Mac 签名公证**：在仓库 Secrets 配置 `APPLE_CERTIFICATE` / `APPLE_CERTIFICATE_PASSWORD` / `APPLE_ID` / `APPLE_PASSWORD` / `APPLE_TEAM_ID`，CI 自动 Developer ID 签名并公证。
- **Windows**：CI 出安装包，暂不强制代码签名（用户下载会有 SmartScreen 提示，可接受）。

## 目录结构

```
DSonMac/
├─ src/                 # 前端（TypeScript + Vite）
├─ src-tauri/           # Rust 后端
│  ├─ src/{main,db,memory,deepseek,state}.rs
│  ├─ tauri.conf.json
│  └─ capabilities/
├─ legacy/swift-shell/  # 早期 Swift 网页套壳（已归档）
└─ .github/workflows/   # 双端发布 CI
```

## 协议

MIT © wolfprince
