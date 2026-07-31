# DSonDT

> **D**eep**S**eek **on** **D**esktop

本地运行的 **DeepSeek 桌面客户端**，弥补 DeepSeek 网页版「没有长期记忆」和「没有桌面客户端」两个短板。自己持有 API Key 调用 DeepSeek API，全部对话与长期记忆存储在本地，**Mac（仅 Apple Silicon）+ Windows 双端**，开源免费。

## 特性

- 🖥️ 原生桌面客户端（Mac / Windows），UI 完全复刻 DeepSeek 网页端，零学习成本
- 🧠 **长期记忆（可编辑）**：用 DeepSeek embedding 把历史对话转向量存本地，新对话自动检索相关片段注入上下文
  - 自动记忆：你每次发消息都会自动沉淀为记忆
  - 手动记忆：在「🧠 记忆库」里自己添加 / 编辑 / 删除，掌控 AI 对你的长期认知
- 🔑 API Key 存入系统钥匙串（macOS 钥匙串 / Windows 凭据管理器），不落明文、不上传
  - 设置页一键跳转 https://platform.deepseek.com/api_keys 获取 Key，复制粘贴即可
- 💬 多会话管理、流式输出、深度思考开关（`thinking`）
- 📦 对话导入 / 导出（JSON）
- 🆓 开源免费（MIT），低维护（一套 Tauri 代码出双端，CI 自动构建）

## 为什么没有「扫码/账号登录自动导入 Key」

DeepSeek 目前**只支持 API Key 鉴权**，官方不提供 OAuth / 账号授权换取 API Key 的接口，因此无法做到「登录账号后自动拉取 Key」。退而求其次，设置页提供**一键跳转获取 Key** 的按钮，复制粘贴即可。

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

首次启动在「设置」中填入 DeepSeek API Key（点「去 DeepSeek 获取 API Key」按钮跳转获取）。

## 发布（双端 CI）

打 tag 即可触发 GitHub Actions，自动构建 Mac(arm64) 与 Windows(x64) 并发布 Release：

```bash
git tag v0.1.0
git push origin v0.1.0
```

下载安装包请到 GitHub Releases（打 tag 自动发布）。

- **Mac（不做签名公证）**：项目无 Apple 开发者账号，CI 出 **ad-hoc 签名**的 `.app` / `.dmg`，安装后首次打开会被 Gatekeeper 拦截「无法验证开发者」。三种绕过方式任选其一：
  1. 右键 App →「打开」；
  2. 终端执行 `xattr -cr /Applications/DSonDT.app` 后再打开；
  3. 系统设置 → 隐私与安全性 → 点「仍要打开」。
- **Windows**：CI 出安装包，暂不强制代码签名（用户下载会有 SmartScreen 提示，可接受）。

## 目录结构

```
DSonDT/
├─ src/                 # 前端（TypeScript + Vite）
│  ├─ src/{ui,api,store,types}.ts
│  └─ src/style.css
├─ src-tauri/           # Rust 后端
│  ├─ src/{main,db,memory,deepseek,state}.rs
│  ├─ tauri.conf.json
│  └─ capabilities/
├─ legacy/swift-shell/  # 早期 Swift 网页套壳（已归档）
└─ .github/workflows/   # 双端发布 CI
```

## 协议

MIT © wolfprince
