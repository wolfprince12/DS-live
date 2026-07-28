# DSonMac 技术方案与 MVP 计划

> 版本：v0.1 草案 ｜ 日期：2026-07-25 ｜ 状态：待确认后开工

## 1. 产品定位（一句话）
本地运行的 DeepSeek 桌面客户端：用户自带 API Key，本地存储全部对话并具备**长期记忆**（弥补网页版无记忆短板），Mac（仅 Apple Silicon）+ Windows 双端，开源免费、低维护。

## 2. 技术架构（已定决策）
- **框架**：Tauri 2.x（Rust 后端 + Web 前端），一套代码出双端，二进制小（几 MB）、内存占用低、开源友好、维护量最低。
- **前端**：轻量 Web 技术。为"低维护 + 你可读可改"，建议原生 TypeScript + 极简 UI（或 Svelte）；不引入重框架。
- **后端（Rust）**：
  - DeepSeek API 流式调用（SSE，chat/completions）
  - 本地存储：SQLite（对话、消息、记忆向量）
  - 记忆检索：DeepSeek embeddings API 生成向量 + 本地余弦相似度 Top-K
  - 密钥安全：存系统钥匙串（macOS Keychain / Windows Credential Manager），不落明文
- **AI 能力**：
  - 对话：`deepseek-v4-flash`（非思考）/ `deepseek-v4-pro`（思考，默认开启 `thinking`）
  - 记忆 embedding：DeepSeek `embeddings` 接口（成本极低）
- **跨平台构建**：GitHub Actions CI，push tag 自动构建 Mac(arm64) + Windows(x64) 并发布 Release，你不用碰 Windows 机器。

## 3. 现有 Swift 代码处置
当前 `DSonMac/` 下的 Swift 工程只是网页套壳、且编不过，与目标不符。采用 Tauri 后：
- 旧 Swift 代码移入 `legacy/swift-shell/` 归档保留（不删除，尊重你的原始工作），新工程在仓库根直接搭建 `src/` + `src-tauri/`。

## 4. MVP 功能清单（v0.1，建议首版范围）
- [ ] **UI 完全复刻 DeepSeek 网页端**（左侧会话栏 + 主聊天区 + 底部输入栏 + 顶栏），降低用户使用疏离感
- [ ] 首次启动引导配置 API Key（写入系统钥匙串，不落明文）
- [ ] 多会话聊天 UI：新建 / 切换 / 重命名 / 删除对话
- [ ] 流式输出（SSE 逐字显示）
- [ ] 模型选择（deepseek-v4-flash / deepseek-v4-pro，深度思考开关 `thinking`）
- [ ] 本地持久化全部对话与消息（SQLite）
- [ ] **长期记忆**：每条用户消息入库时生成 embedding；新对话/每轮自动检索 Top-K 相关历史片段注入 system prompt
- [ ] 主题：浅色 / 深色 / 跟随系统（沿用你已有的偏好逻辑）
- [ ] 对话导入 / 导出（JSON，开源用户友好）
- [ ] 关于页：GitHub 仓库链接、版本号、开源协议（MIT）

## 5. 目录结构（Tauri 标准）
```
DSonMac/
├─ src/                 # 前端（TS + 极简 UI）
├─ src-tauri/           # Rust 后端
│  ├─ Cargo.toml
│  ├─ tauri.conf.json   # 窗口/打包/签名配置
│  ├─ src/
│  │  ├─ main.rs
│  │  ├─ deepseek.rs    # API 流式调用
│  │  ├─ db.rs          # SQLite
│  │  └─ memory.rs      # embedding + 检索
├─ legacy/swift-shell/  # 归档旧 Swift 代码
└─ .github/workflows/release.yml
```

## 6. 发布与签名（已定：放弃 Mac 签名，Windows 走 CI）
- **Mac**：**不做苹果签名与公证**（用户无 Apple 开发者账号——原账号已被苹果清理，且不愿再付年费）。
  - CI 自动退化为 **ad-hoc 签名**出 `.app` + `.dmg`，可安装，但首次打开会被 Gatekeeper 拦截"无法验证开发者"。
  - 用户侧绕过方式三选一：① 右键 App →「打开」；② 终端 `xattr -cr /Applications/DSonMac.app`；③ 系统设置 → 隐私与安全性 → 点「仍要打开」。
  - 对开源免费工具属可接受体验；若未来购买 Developer Program 账号，重新在 release.yml 配置 `APPLE_*` Secrets 即可恢复公证。
- **Windows**：CI 出 `.msi`（WiX）或 `.exe`（NSIS）；**暂不强签名**（用户下载有 SmartScreen 提示，可接受）；GitHub Releases 分发。
- 全程 GitHub Actions：打 tag 即构建双端并发布 Release（release.yml 已移除 Mac 签名 env）。

## 7. 非 MVP（后续可选）
- 记忆可视化 / 手动编辑 / 清理
- 本地对话全文搜索
- 多 API 兼容（OpenAI 格式，便于换模型）
- 自动更新（tauri-updater）
- Windows 代码签名（消除 SmartScreen 警告，需年费证书）

## 8. 风险与注意
- DeepSeek API 稳定性 / 费率：对话按 token 计费；embedding 极便宜。
- GitHub Free 的 Mac CI 分钟数较少且贵：**仅打 tag 时触发构建**，日常开发本地 `tauri dev` / `tauri build`。
- 本地环境需：Rust 工具链 + Xcode Command Line Tools（Mac 侧）；Windows 构建完全由 CI 完成。

## 9. 下一步
确认本计划后，我先做：脚手架 Tauri 工程（Mac + Win 配置）+ 任务拆解，再逐块实现 MVP。
