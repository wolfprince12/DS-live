<div align="center">

# DSonDT

**DeepSeek on Desktop**

![DSonDT](src-tauri/app-icon-source.png)

本地运行的 **DeepSeek 桌面客户端** —— 自带长期记忆、用户自带 API Key，**所有对话与记忆只存本机**。
macOS（Apple Silicon）原生 · 开源免费 · 零服务端依赖。

[下载安装](#-安装macos) · [特性](#-特性) · [两种模式](#-两种模式共用同一记忆库) · [关于作者](#-关于作者) · [支持作者](#-支持作者)

</div>

---

DSonDT 弥补 DeepSeek 网页端在「**没有长期记忆**」与「**没有桌面客户端**」两个短板：

- 🧠 自带**可编辑的本地长期记忆库** —— 让 AI 真的「记得」你
- 🔑 你自己的 DeepSeek API Key —— 系统钥匙串保管，**零服务端成本**
- 💬 单窗口双模式 —— 网页模式（用你自己的 DeepSeek 账号）+ API 模式（自备 Key）

## ✨ 特性

- 🖥️ **原生 macOS 桌面客户端**（Tauri 2 · Rust 后端 + 原生 TypeScript 前端）
- 🧠 **长期记忆（可编辑）**：在「🧠 记忆库」里自己添加 / 编辑 / 删除，掌控 AI 对你的长期认知
  - **自动记忆**：两种模式下你发出的消息都会自动沉淀
  - **双路检索**：有 API Key 时走 embedding 向量检索；**无 Key 时自动退化为字符二元组关键词检索**，零成本可用
- 🔑 **API Key 持久化保存，重启不丢**：优先写入 macOS 钥匙串；因本地 ad-hoc 签名每次构建的二进制指纹会变、钥匙串可能拒绝新二进制读取，**自动回退到本地 XOR 加密文件**（`apikey.bin`，权限 600），不落明文、不上传
  - 设置页一键跳转 <https://platform.deepseek.com/api_keys> 获取 Key
- 💬 多会话管理、流式输出、深度思考开关（`thinking`）
- 📦 对话导入 / 导出（JSON）

## 🌐 两种模式，共用同一记忆库

首次启动会让你选，之后可在**侧边栏顶部**的「🌐 网页 / 🔑 API」标签随时切换 —— 两种模式都在**同一个窗口**内。

| | 🌐 **网页模式**（默认推荐） | 🔑 **API 模式** |
|---|---|---|
| 花钱 | **不花钱**，用你自己的 DeepSeek 账号 | 按 token 计费，需自备 API Key |
| 界面 | 就是官网原版，官方改版自动跟上 | 本客户端复刻的 UI |
| 历史对话 | 天然全在，且与手机端同步 | 只在本地，另有 JSON 导入导出 |
| 记忆注入 | 发送前**前置注入**到消息开头（你自己按回车） | 以 `system` 身份注入，模型遵循度更高 |
| 稳定性 | 依赖官网页面结构，改版可能需要适配 | 只依赖公开 API，不受官网改版影响 |

**网页模式怎么用**：点侧边栏「🌐 网页」标签，官方页面会内嵌在窗口右侧、登录态长期保留。想让 AI 记得你时，在输入框打完字后点右下角 **🧠 注入记忆**（或按 `⌘ M`），程序会把最匹配的几条本地记忆拼到消息开头 —— **发送键仍由你自己按**，程序不会替你发消息。

## 📦 安装（macOS）

前往 [Releases](https://github.com/wolfprince12/DSonDT/releases) 下载最新 `DSonDT.dmg`。

1. 双击打开 DMG（卷图标就是 DSonDT 的鲸鱼+大脑 logo）
2. 把 `DSonDT.app` 拖进「应用程序」
3. **双击 DMG 里的 `fix.command`**，输入开机密码一键移除隔离限制
4. 在「应用程序」里**右键 `DSonDT.app` → 打开**（首次需绕过 Gatekeeper）

> 项目无 Apple 开发者账号，CI 出 **ad-hoc 签名**的 `.app`，首次打开会被 Gatekeeper 拦截「无法验证开发者」。除上述 `fix.command` 一键法外，也可：① 系统设置 → 隐私与安全性 → 点「仍要打开」；② 终端 `xattr -cr /Applications/DSonDT.app`。

## 🛠 开发构建

> 前置：Rust 工具链 + Xcode Command Line Tools + Node.js 20+

```bash
cd src && npm install        # 前端依赖
npm run tauri dev            # 开发模式（热更新）
npm run tauri build          # 打包：.app + .dmg
```

首次启动在「设置」中填入 DeepSeek API Key。

## 👋 关于作者

**大狼（Winter Zheng）** —— 二十年影视传媒老兵，做过音乐节、纪录片、综艺导演，作品上过央视春晚、北影节音乐节。八年前独立运营「大狼导演工作室」，近年转型用 AI 把跨界底子焊成产品：爻知 AI、DealV、桂海晴岚音乐节、瘪老二作妖记 ……

DSonDT 最初是给自己做的工具 —— 二十年跨界干下来，AI 是唯一能同时记住所有项目细节、所有合作方偏好、还能随时调用回来的助手。后来觉得「想让 AI 真的记得自己」这件事大家都需要，就开源了出来。

云珩（北京）文化传媒有限公司 出品。

## ☕ 支持作者

如果 DSonDT 对你有帮助，欢迎扫码赞助一杯咖啡 —— 所有赞助都会变成 DeepSeek API token，继续测试和迭代。

![支付二维码](assets/payment-qr.png)

也可以留个 Star ⭐，这是对独立开发者最大的鼓励。

## 📜 协议

MIT © wolfprince