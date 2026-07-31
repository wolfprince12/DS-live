# DSonDT 技术方案与 MVP 计划

> 版本：v0.3.0 ｜ 日期：2026-07-31 ｜ 状态：macOS 首发版，持续打磨中

## 1. 产品定位（一句话）
本地运行的 DeepSeek 桌面客户端：用户自带 API Key，本地存储全部对话并具备**长期记忆**（弥补网页版无记忆短板），macOS（仅 Apple Silicon）原生，开源免费、低维护。

## 2. 技术架构（已定决策）
- **框架**：Tauri 2.x（Rust 后端 + Web 前端），二进制小（几 MB）、内存占用低、开源友好、维护量最低。
- **前端**：轻量 Web 技术。为"低维护 + 你可读可改"，建议原生 TypeScript + 极简 UI（或 Svelte）；不引入重框架。
- **后端（Rust）**：
  - DeepSeek API 流式调用（SSE，chat/completions）
  - 本地存储：SQLite（对话、消息、记忆向量）
  - 记忆检索：DeepSeek embeddings API 生成向量 + 本地余弦相似度 Top-K
  - 密钥安全：优先存 macOS 钥匙串；因 ad-hoc 签名二进制指纹每次构建都会变化、钥匙串 ACL 可能拒绝新二进制读取，**自动回退到本地 XOR 加密文件**（`apikey.bin`，权限 600），均不落明文
- **AI 能力**：
  - 对话：`deepseek-v4-flash`（非思考）/ `deepseek-v4-pro`（思考，默认开启 `thinking`）
  - 记忆 embedding：DeepSeek `embeddings` 接口（成本极低）
- **构建发布**：GitHub Actions CI，push tag 自动构建 Mac(arm64) 并发布 Release。

## 3. 现有 Swift 代码处置
原 `DSonMac/` 下的 Swift 工程只是网页套壳、且编不过，与目标不符。采用 Tauri 后：
- 旧 Swift 代码已移入 `legacy/swift-shell/` 归档保留（不删除，尊重你的原始工作），新工程在仓库根搭建 `src/` + `src-tauri/`，全局软件名已统一为 **DSonDT（DeepSeek on Desktop）**。

## 4. MVP 功能清单（v0.1 → v0.3.0 进度）
- [x] **UI 完全复刻 DeepSeek 网页端**（左侧会话栏 + 主聊天区 + 底部输入栏 + 顶栏），降低用户使用疏离感
- [x] 首次启动引导配置 API Key（写入系统钥匙串，不落明文）；设置页提供**一键跳转获取 Key** 按钮
- [x] 多会话聊天 UI：新建 / 切换 / 重命名 / 删除对话
- [x] 流式输出（SSE 逐字显示）
- [x] 模型选择（deepseek-v4-flash / deepseek-v4-pro，深度思考开关 `thinking`）
- [x] 本地持久化全部对话与消息（SQLite）
- [x] **长期记忆（自动）**：每条用户消息入库时生成 embedding；新对话/每轮自动检索 Top-K 相关历史片段注入 system prompt
- [x] **长期记忆（可编辑）**：新增「🧠 记忆库」面板，支持手动添加 / 编辑 / 删除记忆，自动与手动来源以标签区分（`auto` / `manual`），记忆表独立于消息表
- [x] **单窗口双模式**：网页模式与 API 模式同处一个窗口（侧边栏顶部「🌐 网页 / 🔑 API」标签切换），网页模式下官方页面内嵌于窗口右侧、不再弹出第二个窗口；记忆库可在网页模式下直接打开编辑
- [x] **API Key 持久化**：优先写入系统钥匙串，ad-hoc 签名导致二进制指纹变化、钥匙串读取失败时**自动回退本地 XOR 加密文件**，重启后不再丢失
- [x] 主题：浅色 / 深色 / 跟随系统（沿用你已有的偏好逻辑）
- [x] 对话导入 / 导出（JSON，开源用户友好）
- [x] 关于页：GitHub 仓库链接、版本号、开源协议（MIT）
- [x] **macOS 原生拖动**：CSS `-webkit-app-region: drag` 整窗可拖，交互元素反向覆盖
- [x] **网页模式记忆前置注入**：🧠 注入记忆按钮（带角标 + ⌘M 快捷键），React 受控组件兼容
- [x] **DMG 卷图标 + fix.command**：UDRW + `SetFile -a C` 加 logo，附一键解除 Gatekeeper 隔离脚本

## 5. 目录结构（Tauri 标准）
```
DSonDT/
├─ src/                 # 前端（TS + 极简 UI）
│  ├─ src/{ui,api,store,types}.ts
│  └─ src/style.css
├─ src-tauri/           # Rust 后端
│  ├─ Cargo.toml
│  ├─ tauri.conf.json   # 窗口/打包/签名配置
│  ├─ src/
│  │  ├─ main.rs         # 命令注册
│  │  ├─ deepseek.rs    # API 流式调用 + embedding
│  │  ├─ db.rs          # SQLite（conversations/messages/memories 三表）
│  │  ├─ memory.rs      # 余弦相似度检索
│  │  ├─ state.rs       # 应用状态 + 记忆写入 + API Key 持久化（钥匙串/本地加密文件）
│  │  └─ webmode.rs     # 网页模式：同一窗口内双 WebView（本地 UI + 官方页面）+ 记忆注入
├─ legacy/swift-shell/  # 归档旧 Swift 代码
└─ .github/workflows/release.yml
```

## 6. 发布与签名（已定：放弃 Mac 签名公证）
- **Mac**：**不做苹果签名与公证**（用户无 Apple 开发者账号——原账号已被苹果清理，且不愿再付年费）。
  - CI 自动退化为 **ad-hoc 签名**出 `.app` + `.dmg`，可安装，但首次打开会被 Gatekeeper 拦截"无法验证开发者"。
  - 用户侧绕过方式三选一：① 右键 App →「打开」；② 双击 DMG 里的 `fix.command` 一键 `xattr -cr`；③ 系统设置 → 隐私与安全性 → 点「仍要打开」。
  - 对开源免费工具属可接受体验；若未来购买 Developer Program 账号，重新在 release.yml 配置 `APPLE_*` Secrets 即可恢复公证。
- **macOS DMG**：手工 UDRW + `SetFile -a C` 流程加卷图标（参考鼠须管 Squirrel）；DMG 内含 `fix.command` 一键解除 Gatekeeper 隔离；GitHub Releases 分发。
- 全程 GitHub Actions：打 tag 即构建并发布 Release（release.yml 已移除 Mac 签名 env）。

## 7. 关于「登录关联账号自动导入 Key」的说明
需求曾希望「设置里登录 DeepSeek 账号后自动导入 API Key」。经评估：**DeepSeek 仅支持 API Key 鉴权，官方未提供 OAuth / 账号授权换取 Key 的接口**，技术上无法实现自动导入。已采用务实替代方案——设置页放「去 DeepSeek 获取 API Key」一键跳转按钮，复制粘贴即可。如未来 DeepSeek 开放授权接口，可再加「登录」入口。

## 8. 非 MVP（后续可选）
- 记忆可视化（关系图谱 / 时间线）
- 本地对话全文搜索
- 多 API 兼容（OpenAI 格式，便于换模型）
- 自动更新（tauri-updater）
- Apple 开发者账号 + 公证（消除 Gatekeeper 拦截，需年费）

## 9. 风险与注意
- DeepSeek API 稳定性 / 费率：对话按 token 计费；embedding 极便宜。
- GitHub Free 的 Mac CI 分钟数较少且贵：**仅打 tag 时触发构建**，日常开发本地 `tauri dev` / `tauri build`。
- 本地环境需：Rust 工具链 + Xcode Command Line Tools。

## 10. 下一步
v0.3.0 已实质落地：单窗口双视图、本地长期记忆（自动 + 可编辑）、macOS 拖动与卷图标、网页模式记忆注入均已就绪。下一步聚焦：打首个公开 Release tag、收集使用反馈、迭代记忆检索质量。