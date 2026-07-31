# DSonDT

> **D**eep**S**eek **on** **D**esktop

本地运行的 **DeepSeek 桌面客户端**，弥补 DeepSeek 网页版「没有长期记忆」和「没有桌面客户端」两个短板。全部对话与长期记忆**只存本地**，**Mac（仅 Apple Silicon）+ Windows 双端**，开源免费。

## 两种模式，共用同一个本地记忆库

首次启动会让你选，之后可在**左侧边栏顶部**的「🌐 网页 / 🔑 API」标签随时切换——两种模式都在**同一个窗口**内，不会弹出第二个窗口。

| | 🌐 **网页模式**（默认推荐） | 🔑 **API 模式** |
|---|---|---|
| 花钱 | **不花钱**，用你自己的 DeepSeek 账号 | 按 token 计费，需自备 API Key |
| 界面 | 就是官网原版，官方改版自动跟上 | 本客户端复刻的 UI |
| 历史对话 | 天然全在，且与手机端同步 | 只在本地，另有 JSON 导入导出 |
| 记忆注入 | 发送前**前置注入**到消息开头（你自己按回车） | 以 `system` 身份注入，模型遵循度更高 |
| 稳定性 | 依赖官网页面结构，改版可能需要适配 | 只依赖公开 API，不受官网改版影响 |

**网页模式怎么用**：点侧边栏顶部的「🌐 网页」标签，官方页面会内嵌在窗口右侧、登录态长期保留。想让 AI 记得你时，在输入框打完字后点右下角 **🧠 注入记忆**（或按 `Cmd/Ctrl + M`），程序会把最匹配的几条本地记忆拼到消息开头 —— **发送键仍然由你自己按**，程序不会替你发消息。同时右侧的「📚 编辑记忆库」按钮可**直接在本窗口打开记忆库**编辑，无需切回 API 模式。

## 特性

- 🖥️ 原生桌面客户端（Mac / Windows）
- 🧠 **长期记忆（可编辑）**：在「🧠 记忆库」里自己添加 / 编辑 / 删除，掌控 AI 对你的长期认知
  - 自动记忆：两种模式下你发出的消息都会自动沉淀
  - 检索**双路**：有 API Key 时走 embedding 向量检索；**没有 Key 时自动退化为字符二元组关键词检索**，零成本可用
- 🔑 API Key **持久化保存，重启不丢**：优先写入系统钥匙串（macOS 钥匙串 / Windows 凭据管理器）；因本地 ad-hoc 签名每次构建的二进制指纹会变、钥匙串可能拒绝新二进制读取，**自动回退到本地 XOR 加密文件**（`apikey.bin`，权限 600），二者都不落明文、不上传
  - 设置页一键跳转 https://platform.deepseek.com/api_keys 获取 Key
- 💬 多会话管理、流式输出、深度思考开关（`thinking`）
- 📦 对话导入 / 导出（JSON）
- 🆓 开源免费（MIT），低维护（一套 Tauri 代码出双端，CI 自动构建）

## 关于边界

- **没有「登录自动导入 API Key」**：DeepSeek 只支持 API Key 鉴权，官方不提供 OAuth / 账号授权换 Key 的接口。所以 API 模式只能一键跳转 + 手动粘贴。
- **网页模式不逆向、不代发**：注入脚本只做两件事 —— 把记忆文本填进输入框（等同输入法辅助），以及**被动旁听**页面自己发出的请求以沉淀记忆。不伪造请求、不窃取 token、不自动点发送。
- **网页模式的记忆是「前缀」不是「system」**：模型是否采纳取决于它自己，实测命中率高，但结构性权重不如 API 模式的 system prompt。这是这条路唯一实质性的能力折损。

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
│  ├─ src/webmode.rs    # 网页模式：同一窗口内双 WebView（本地 UI + 官方页面）+ 记忆注入脚本
│  ├─ tauri.conf.json
│  └─ capabilities/     # default.json（主窗口）/ webmode.json（远程域名 IPC）
├─ legacy/swift-shell/  # 早期 Swift 网页套壳（已归档）
└─ .github/workflows/   # 双端发布 CI
```

## 协议

MIT © wolfprince
