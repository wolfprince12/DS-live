import { api } from "./api";
import { store } from "./store";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Conversation, Memory, UpdateInfo } from "./types";

let convListEl: HTMLElement;
let messagesEl: HTMLElement;
let inputEl: HTMLTextAreaElement;
let modelSelect: HTMLSelectElement;
let memoryToggle: HTMLInputElement;
let thinkToggle: HTMLInputElement;
let settingsModal: HTMLElement;
let aboutModal: HTMLElement;
let apiKeyInput: HTMLInputElement;
let themeSelect: HTMLSelectElement;
let sendBtn: HTMLButtonElement;
let memoryModal: HTMLElement;
let memoryListEl: HTMLElement;
let memorySearchEl: HTMLInputElement;
let memoryNewArea: HTMLElement;
let memoryNewInput: HTMLTextAreaElement;
let updateModal: HTMLElement;
/** 当前待处理的更新信息（弹窗上的按钮要用） */
let pendingUpdate: UpdateInfo | null = null;
/** 关于弹窗里检查到的新版本（banner 上的「查看更新」要用） */
let aboutPendingUpdate: UpdateInfo | null = null;
/** 当前软件版本号（来自 Rust cargo 版本），关于弹窗展示用 */
let appVersion = "";

const MODELS = [
  { id: "deepseek-v4-flash", name: "DeepSeek V4 Flash" },
  { id: "deepseek-v4-pro", name: "DeepSeek V4 Pro" },
];

const DEEPSEEK_KEY_URL = "https://platform.deepseek.com/api_keys";

// 窗口拖动：macOS 上 titleBarStyle=Overlay（透明标题栏叠加）会让 CSS 的
// -webkit-app-region: drag 失效，故改用 Tauri 的 startDragging()（JS 在
// mousedown 手势中触发原生拖拽），对叠加标题栏 100% 可靠。
// 顶栏/侧栏的可交互子元素（按钮、会话项等）不触发拖动。
const NO_DRAG_SELECTOR =
  'button, input, select, textarea, a, [contenteditable="true"], .no-drag, .mode-tab, .topbar-icon-btn, .win-ctrl-btn, .conv-item, .new-chat-btn';

function enableDrag(selector: string) {
  const el = document.querySelector(selector) as HTMLElement | null;
  if (!el) return;
  el.addEventListener("mousedown", (e: MouseEvent) => {
    if (e.button !== 0) return; // 仅响应左键
    if ((e.target as HTMLElement).closest(NO_DRAG_SELECTOR)) return; // 交互元素不拖
    e.preventDefault();
    getCurrentWindow()
      .startDragging()
      .catch(() => {});
  });
}

export async function initUI() {
  // —— 诊断浮层 ——
  // Windows 虚拟机里没法开 DevTools，这里把任何未捕获的 JS 报错 / Promise 拒绝
  // 显示到屏幕顶部红条，方便盲调。pointer-events:none 保证它自己不挡点击。
  const diagBox = document.createElement("div");
  diagBox.id = "diag-box";
  diagBox.style.cssText =
    "position:fixed;left:0;right:0;top:0;z-index:99999;max-height:42%;overflow:auto;" +
    "background:#b00020;color:#fff;font:12px/1.5 ui-monospace,Menlo,Consolas,monospace;" +
    "padding:8px 12px;white-space:pre-wrap;pointer-events:none;display:none;";
  document.body.appendChild(diagBox);
  const pushDiag = (msg: string) => {
    diagBox.style.display = "block";
    diagBox.textContent += msg + "\n";
  };
  window.addEventListener("error", (e) =>
    pushDiag(`[error] ${e.message}${e.filename ? ` @ ${e.filename}:${e.lineno}` : ""}`),
  );
  window.addEventListener("unhandledrejection", (e) =>
    pushDiag(
      `[promise] ${e.reason instanceof Error ? e.reason.stack || e.reason.message : String(e.reason)}`,
    ),
  );

  const app = document.getElementById("app")!;
  app.innerHTML = template();
  // 平台标识：决定顶栏是否显示自绘窗口控制按钮。
  //   - macOS：titleBarStyle=Overlay 把原生红黄绿浮在顶栏左侧 → platform-mac，无需自绘按钮
  //   - Windows：decorations=false 完全去掉原生标题栏 → platform-windows，顶栏右侧自绘 min/max/close
  //   - 其他（Linux 等）→ platform-linux，同 Windows 策略兜底
  // 两端都用「无原生标题栏 + 本地 UI 自绘 48px 顶栏（logo+模式标签+🧠⚙ℹ）」，App 自身 UI 与 macOS 完全一致；
  // 唯一差异是 OS 窗口控制按钮：mac 浮在左、Windows 自绘在右（不可避免）。
  const nav = (navigator.platform || navigator.userAgent || "").toLowerCase();
  if (/mac/.test(nav)) document.body.classList.add("platform-mac");
  else if (/win/.test(nav)) document.body.classList.add("platform-windows");
  else document.body.classList.add("platform-linux");
  // 拖动：JS 在顶栏/侧栏的 mousedown 上触发 startDragging()（见 enableDrag）
  enableDrag(".app-topbar");
  enableDrag(".sidebar");
  convListEl = document.getElementById("conv-list")!;
  messagesEl = document.getElementById("messages")!;
  inputEl = document.getElementById("input") as HTMLTextAreaElement;
  modelSelect = document.getElementById("model-select") as HTMLSelectElement;
  memoryToggle = document.getElementById("memory-toggle") as HTMLInputElement;
  thinkToggle = document.getElementById("think-toggle") as HTMLInputElement;
  settingsModal = document.getElementById("settings-modal")!;
  aboutModal = document.getElementById("about-modal")!;
  apiKeyInput = document.getElementById("api-key-input") as HTMLInputElement;
  themeSelect = document.getElementById("theme-select") as HTMLSelectElement;
  sendBtn = document.getElementById("send-btn") as HTMLButtonElement;
  memoryModal = document.getElementById("memory-modal")!;
  memoryListEl = document.getElementById("memory-list")!;
  memorySearchEl = document.getElementById("memory-search") as HTMLInputElement;
  memoryNewArea = document.getElementById("memory-new")!;
  memoryNewInput = document.getElementById("memory-new-input") as HTMLTextAreaElement;
  updateModal = document.getElementById("update-modal")!;

  MODELS.forEach((m) => {
    const o = document.createElement("option");
    o.value = m.id;
    o.textContent = m.name;
    modelSelect.appendChild(o);
  });
  modelSelect.value = store.model;
  memoryToggle.checked = store.useMemory;
  thinkToggle.checked = store.thinking;
  themeSelect.value = store.theme;

  document.getElementById("new-chat")!.addEventListener("click", () => newChat());
  sendBtn.addEventListener("click", () => void send());
  inputEl.addEventListener("keydown", (e) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      void send();
    }
  });
  inputEl.addEventListener("input", autoGrow);
  modelSelect.addEventListener("change", () => store.setModel(modelSelect.value));
  memoryToggle.addEventListener("change", () => store.setMemory(memoryToggle.checked));
  thinkToggle.addEventListener("change", () => store.setThinking(thinkToggle.checked));
  themeSelect.addEventListener("change", () => store.setTheme(themeSelect.value));
  document.getElementById("tab-web")!.addEventListener("click", () => void switchMode("web"));
  document.getElementById("tab-api")!.addEventListener("click", () => void switchMode("api"));
  // 顶栏按钮：记忆库 / 设置 / 关于
  document.getElementById("memory-btn")!.addEventListener("click", () => void openMemory());
  document.getElementById("settings-btn")!.addEventListener("click", () => void openSettings());
  document.getElementById("about-btn")!.addEventListener("click", () => void openAbout());
  // Windows 自绘窗口控制按钮（decorations=false 时由本地 UI 负责 min/max/close）
  // 走 @tauri-apps/api/window 的同名方法，与 macOS 的 JS 拖动 API 同源、无需新增 Rust 命令。
  try {
    const win = getCurrentWindow();
    document.getElementById("win-min")?.addEventListener("click", () => void win.minimize().catch(() => {}));
    document.getElementById("win-max")?.addEventListener("click", () => void win.toggleMaximize().catch(() => {}));
    document.getElementById("win-close")?.addEventListener("click", () => void win.close().catch(() => {}));
  } catch (e) {
    // 极端情况下拿不到窗口句柄也不要让 initUI 中断（否则模态框内部按钮等监听都不会挂载）
    pushDiag(`[win-ctrl] ${e}`);
  }
  // 网页模式里注入的「📚 编辑记忆库」按钮会经 Rust 派发这个事件，由本地 UI 打开弹窗
  window.addEventListener("dsondt:open-memory", () => void openMemory());
  // Esc 关闭当前打开的模态框
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      if (!updateModal.hidden) closeUpdate();
      else if (!aboutModal.hidden) closeAbout();
      else if (!settingsModal.hidden) closeSettings();
      else if (!memoryModal.hidden) closeMemory();
    }
  });
  document.getElementById("settings-cancel")!.addEventListener("click", () => closeSettings());
  document.getElementById("settings-save")!.addEventListener("click", saveSettings);
  document.getElementById("get-key-btn")!.addEventListener("click", () => void openKeyPage());
  document.getElementById("promo-dealv-btn")!.addEventListener("click", () => void api.openUrl("https://dealv.cn"));
  document.getElementById("promo-squirrel-btn")!.addEventListener("click", () => void api.openUrl("https://github.com/wolfprince12/squirrel-Panel"));
  // 顶栏菜单项（已拆为独立按钮，菜单下拉取消）
  // 关于弹窗
  document.getElementById("about-close")!.addEventListener("click", () => closeAbout());
  document.getElementById("about-check-update-btn")!.addEventListener("click", () => void manualCheckUpdateAbout());
  document.getElementById("about-update-go")!.addEventListener("click", () => {
    if (aboutPendingUpdate) showUpdate(aboutPendingUpdate);
  });
  document.getElementById("about-github-btn")!.addEventListener("click", () =>
    void api.openUrl("https://github.com/wolfprince12/DSonDT"),
  );
  document.getElementById("about-release-btn")!.addEventListener("click", () =>
    void api.openUrl("https://github.com/wolfprince12/DSonDT/releases"),
  );
  document.getElementById("update-later")!.addEventListener("click", () => closeUpdate());
  document.getElementById("update-skip")!.addEventListener("click", () => {
    if (pendingUpdate) localStorage.setItem(SKIP_VERSION_KEY, pendingUpdate.latest);
    closeUpdate();
  });
  document.getElementById("update-go")!.addEventListener("click", () => {
    if (pendingUpdate?.download_url) void api.openUrl(pendingUpdate.download_url);
    closeUpdate();
  });
  document.getElementById("export-btn")!.addEventListener("click", exportCurrent);
  document.getElementById("import-btn")!.addEventListener("click", () => document.getElementById("import-file")!.click());
  document.getElementById("import-file")!.addEventListener("change", importFile);
  document.getElementById("memory-close")!.addEventListener("click", () => closeMemory());
  document.getElementById("memory-add")!.addEventListener("click", () => {
    memoryNewArea.hidden = !memoryNewArea.hidden;
    if (!memoryNewArea.hidden) memoryNewInput.focus();
  });
  document.getElementById("memory-new-cancel")!.addEventListener("click", () => {
    memoryNewInput.value = "";
    memoryNewArea.hidden = true;
  });
  document.getElementById("memory-new-save")!.addEventListener("click", () => void addMemory());
  memorySearchEl.addEventListener("input", renderMemories);

  document.getElementById("mode-web")!.addEventListener("click", () => {
    document.getElementById("mode-modal")!.hidden = true;
    document.body.classList.remove("modal-open");
    store.setMode("web");
    updateModeTabs();
    void enterWebMode();
  });
  document.getElementById("mode-api")!.addEventListener("click", async () => {
    document.getElementById("mode-modal")!.hidden = true;
    document.body.classList.remove("modal-open");
    store.setMode("api");
    updateModeTabs();
    if (!(await api.hasApiKey())) openSettings();
  });

  if (!store.mode) {
    // 首次启动：先让用户选模式，而不是上来就要 Key
    document.getElementById("mode-modal")!.hidden = false;
    document.body.classList.add("modal-open");
  } else if (store.mode === "api") {
    // 浏览器预览（非 Tauri runtime）时 api.hasApiKey 会抛错，吞掉即可
    try {
      if (!(await api.hasApiKey())) openSettings();
    } catch {
      /* 浏览器预览环境：无 Tauri runtime，忽略 */
    }
  } else if (store.mode === "web") {
    // 上次选了网页模式：重启后顶栏高亮正确，但 deepseek webview 不会被自动创建，
    // 需要主动调 openWebMode() 激活；否则用户会看到「按钮高亮网页、但下面仍是本地 API 界面」
    // 的状态错乱。
    try { void api.openWebMode(); } catch { /* 浏览器预览 */ }
  }
  updateModeTabs();
  try { await store.refreshConversations(); } catch { /* 浏览器预览 */ }
  renderSidebar();
  if (store.conversations.length > 0) {
    try { await selectConversation(store.conversations[0].id); } catch { /* 浏览器预览 */ }
  } else {
    newChat();
  }

  // 取当前版本号（关于弹窗展示用）
  try {
    appVersion = await api.getVersion();
  } catch {
    /* 浏览器预览：无 Tauri runtime，忽略 */
  }

  // 开发预览：#about / #about-update 用假数据打开关于弹窗，便于截图核对样式
  if (window.location.hash.startsWith("#about")) {
    setTimeout(() => {
      openAbout();
      if (window.location.hash === "#about-update") {
        aboutPendingUpdate = demoUpdateInfo(false);
        document.getElementById("about-update-ver")!.textContent = `v${aboutPendingUpdate.latest}`;
        document.getElementById("about-update-banner")!.hidden = false;
      }
    }, 100);
    return;
  }
  // 开发预览：URL hash 为 #settings 时自动打开设置弹窗（便于浏览器/Chrome headless 截屏验证）
  if (window.location.hash === "#settings") {
    setTimeout(() => openSettings(), 100);
  }
  // 开发预览：#update-demo / #update-demo-mirror 用假数据渲染更新弹窗，仅用于截图核对样式
  if (window.location.hash.startsWith("#update-demo")) {
    setTimeout(() => showUpdate(demoUpdateInfo(window.location.hash.endsWith("mirror"))), 100);
    return;
  }

  // 启动检查更新：整个进程生命周期只跑一次，延迟一点让首屏先画出来。
  setTimeout(() => void checkUpdateOnStartup(), 1500);
}

function template(): string {
  return `
  <div class="app">
    <header class="app-topbar">
      <div class="app-brand">
        <img src="/logo.png" class="app-logo" alt="DSonDT">
        <span class="app-name">DSonDT</span>
      </div>
      <div class="mode-switch">
        <button class="mode-tab" id="tab-web" type="button">🌐 网页</button>
        <button class="mode-tab" id="tab-api" type="button">🔑 API</button>
      </div>
      <span class="spacer"></span>
      <button id="memory-btn" class="topbar-icon-btn" title="记忆库">🧠</button>
      <button id="settings-btn" class="topbar-icon-btn" title="设置">⚙</button>
      <button id="about-btn" class="topbar-icon-btn" title="关于 DSonDT">ℹ</button>
      <div class="win-controls" id="win-controls">
        <button id="win-min" class="win-ctrl-btn" title="最小化" type="button">—</button>
        <button id="win-max" class="win-ctrl-btn" title="最大化" type="button">□</button>
        <button id="win-close" class="win-ctrl-btn win-ctrl-close" title="关闭" type="button">✕</button>
      </div>
    </header>
    <div class="app-body">
      <aside class="sidebar">
        <div class="sidebar-newchat">
          <button id="new-chat" class="new-chat-btn">+ 新对话</button>
        </div>
        <div class="conv-list" id="conv-list"></div>
      </aside>
      <main class="main">
        <header class="topbar">
          <select id="model-select" class="model-select"></select>
          <label class="toggle-label"><input type="checkbox" id="memory-toggle" /> 长期记忆</label>
          <label class="toggle-label"><input type="checkbox" id="think-toggle" /> 深度思考</label>
          <span class="spacer"></span>
        </header>
        <div class="messages" id="messages"></div>
        <div class="input-area">
          <div class="input-box">
            <textarea id="input" rows="1" placeholder="给 DSonDT 发送消息…（Enter 发送，Shift+Enter 换行）"></textarea>
            <div class="input-row">
              <button id="export-btn" class="ghost-btn">导出</button>
              <button id="import-btn" class="ghost-btn">导入</button>
              <button id="send-btn" class="send-btn">↑</button>
            </div>
          </div>
        </div>
      </main>
    </div>
  </div>
  <div class="modal-mask" id="settings-modal" hidden>
    <div class="modal settings-modal">
      <h3>设置</h3>
      <label>主题</label>
      <select id="theme-select">
        <option value="light">浅色</option>
        <option value="dark">深色</option>
        <option value="system">跟随系统</option>
      </select>
      <label>DeepSeek API Key</label>
      <input type="password" id="api-key-input" placeholder="sk-..." />
      <div class="tip">Key 仅保存在本地（优先系统钥匙串，不可用时回退到本地加密文件），不会以明文上传。</div>
      <div class="key-status" id="key-status"></div>
      <button id="get-key-btn" class="link-btn">去 DeepSeek 获取 API Key ↗</button>

      <div class="modal-actions">
        <button id="settings-cancel" class="ghost-btn">取消</button>
        <button id="settings-save" class="send-btn" style="width:auto;padding:0 18px;height:36px;">保存</button>
      </div>
    </div>
  </div>
  <div class="modal-mask" id="about-modal" hidden>
    <div class="modal about-modal">
      <div class="about-head">
        <img src="/logo.png" class="about-logo" alt="DSonDT" />
        <div class="about-id">
          <div class="about-name">DSonDT</div>
          <div class="about-tagline">DeepSeek on Desktop · 本地 DeepSeek 桌面客户端</div>
        </div>
        <div class="about-head-actions">
          <button id="about-check-update-btn" class="link-btn">检查更新</button>
        </div>
      </div>

      <div class="version-row" style="border-top:none;padding-top:0;margin-top:0;">
        <span class="version-label" id="about-version-label">版本 —</span>
        <span class="version-status" id="about-version-status"></span>
      </div>

      <div class="update-net about-update-banner" id="about-update-banner" hidden>
        <span>发现新版本 <b id="about-update-ver"></b></span>
        <span class="spacer"></span>
        <button id="about-update-go" class="link-btn">立即更新 ↗</button>
      </div>

      <div class="about-author">
        <div class="about-author-avatar">大</div>
        <div class="about-author-meta">
          <div class="about-author-name">Mr大狼</div>
          <div class="about-author-title">导演 / 制作人 / AI 产品创作者 · 20 余年跨界经验</div>
        </div>
      </div>

      <div class="about-products-heading">更多作品</div>
      <div class="about-products">
        <div class="about-product about-product-featured">
          <div class="about-product-top">
            <div class="about-product-icon" style="background:#1aad19">💬</div>
            <div class="about-product-name">爻知云 AI <span class="about-product-tag">微信服务号</span></div>
            <img class="about-product-qr" src="/yiaozhiyun-qr.png" alt="爻知云 AI 微信搜一搜二维码" />
          </div>
          <div class="about-product-desc">关注公众号获取 AI 创作助手、工作流技巧与项目动态。</div>
        </div>
        <div class="about-product">
          <div class="about-product-top">
            <div class="about-product-icon" style="background:#5b6cff">📄</div>
            <div class="about-product-name">DealV <span class="about-product-tag">AI 智能合同</span></div>
          </div>
          <div class="about-product-desc">面向专业人群的合同智能审查与管理平台。</div>
          <button id="promo-dealv-btn" class="link-btn">访问 DealV ↗</button>
        </div>
        <div class="about-product">
          <div class="about-product-top">
            <div class="about-product-icon" style="background:#e8a33d">⌨</div>
            <div class="about-product-name">鼠须管控制面板 <span class="about-product-tag">开源</span></div>
          </div>
          <div class="about-product-desc">超级实用的第三方鼠须管输入法配置工具。</div>
          <button id="promo-squirrel-btn" class="link-btn">查看项目 ↗</button>
        </div>
      </div>

      <div class="modal-actions">
        <div class="about-links">
          <button id="about-github-btn" class="link-btn">GitHub 仓库 ↗</button>
          <button id="about-release-btn" class="link-btn">更新日志 ↗</button>
        </div>
        <button id="about-close" class="ghost-btn">关闭</button>
      </div>
    </div>
  </div>
  <div class="modal-mask" id="memory-modal" hidden>
    <div class="modal memory-modal">
      <h3>长期记忆库</h3>
      <div class="tip">自动记忆来自你的对话；手动记忆由你本人添加/编辑。所有记忆仅保存在本地数据库，不会上传。</div>
      <input type="text" id="memory-search" class="memory-search" placeholder="搜索记忆…" />
      <div class="memory-list" id="memory-list"></div>
      <div class="memory-new" id="memory-new" hidden>
        <textarea id="memory-new-input" rows="3" placeholder="输入一条要记住的内容…"></textarea>
        <div class="modal-actions">
          <button id="memory-new-cancel" class="ghost-btn">取消</button>
          <button id="memory-new-save" class="send-btn" style="width:auto;padding:0 18px;height:36px;">添加</button>
        </div>
      </div>
      <div class="modal-actions">
        <button id="memory-close" class="ghost-btn">关闭</button>
        <button id="memory-add" class="send-btn" style="width:auto;padding:0 18px;height:36px;">+ 新建记忆</button>
      </div>
    </div>
  </div>
  <div class="modal-mask" id="mode-modal" hidden>
    <div class="modal mode-modal">
      <h3>选择使用方式</h3>
      <div class="tip">两种模式共用同一个本地记忆库，随时可以在顶栏切换。</div>
      <div class="mode-card" id="mode-web">
        <div class="mode-title">🌐 网页模式<span class="menu-tag">推荐 · 免费</span></div>
        <div class="mode-desc">
          内嵌 DeepSeek 官网，用你自己的账号登录。不花一分钱、历史对话与手机端同步、界面就是官方原版。
          发消息前点一下「🧠 注入记忆」即可把本地记忆带进这轮对话。
        </div>
      </div>
      <div class="mode-card" id="mode-api">
        <div class="mode-title">🔑 API 模式</div>
        <div class="mode-desc">
          用你自己的 API Key 直连接口，按 token 计费。记忆以 system 身份注入，模型遵循度更高，
          且完全不受官网改版影响。
        </div>
      </div>
    </div>
  </div>
  <div class="modal-mask" id="update-modal" hidden>
    <div class="modal update-modal">
      <div class="update-head">
        <div class="update-badge">NEW</div>
        <div class="update-head-text">
          <h3>发现新版本</h3>
          <div class="update-ver">
            <span class="update-ver-old" id="update-cur"></span>
            <span class="update-ver-arrow">→</span>
            <span class="update-ver-new" id="update-new"></span>
          </div>
        </div>
      </div>
      <div class="update-net" id="update-net" hidden></div>
      <div class="update-notes-wrap" id="update-notes-wrap" hidden>
        <div class="update-notes-title">更新内容</div>
        <div class="update-notes" id="update-notes"></div>
      </div>
      <div class="update-mirrors" id="update-mirrors" hidden>
        <div class="update-mirrors-title">备用下载线路（上面那个打不开就换一条）</div>
        <div class="update-mirror-list" id="update-mirror-list"></div>
      </div>
      <div class="modal-actions update-actions">
        <button id="update-skip" class="ghost-btn">跳过此版本</button>
        <button id="update-later" class="ghost-btn">稍后提醒</button>
        <button id="update-go" class="send-btn" style="width:auto;padding:0 18px;height:36px;">立即下载 ↗</button>
      </div>
    </div>
  </div>
  <input type="file" id="import-file" accept="application/json" hidden />
  `;
}

function fmtDate(ts: number): string {
  const d = new Date(ts * 1000);
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
}

function renderSidebar() {
  convListEl.innerHTML = "";
  for (const c of store.conversations) {
    const item = document.createElement("div");
    item.className = "conv-item" + (c.id === store.currentId ? " active" : "");
    const title = document.createElement("div");
    title.className = "conv-title";
    title.textContent = c.title;
    const del = document.createElement("button");
    del.className = "conv-del";
    del.textContent = "×";
    del.title = "删除对话";
    del.addEventListener("click", async (e) => {
      e.stopPropagation();
      await api.deleteConversation(c.id);
      await store.refreshConversations();
      renderSidebar();
      if (store.currentId === c.id) newChat();
    });
    item.appendChild(title);
    item.appendChild(del);
    item.addEventListener("click", () => void selectConversation(c.id));
    convListEl.appendChild(item);
  }
}

async function selectConversation(id: number) {
  await store.loadMessages(id);
  renderMessages();
}

function newChat() {
  store.currentId = null;
  store.messages = [];
  renderMessages();
  renderSidebar();
  inputEl.focus();
}

function renderMessages() {
  messagesEl.innerHTML = "";
  if (store.messages.length === 0) {
    const empty = document.createElement("div");
    empty.className = "empty-state";
    empty.innerHTML =
      `<div class="logo">DSonDT</div>` +
      `<div class="hint">本地 DeepSeek 客户端 · 自带长期记忆</div>` +
      (store.mode === "web"
        ? `<div class="hint">你选的是网页模式，DeepSeek 官网已嵌在右侧。` +
          `聊天前点右侧的「🧠 注入记忆」即可把本地记忆带进对话。</div>`
        : `<div class="hint">当前为 API 模式。想用自己的 DeepSeek 账号免费聊？` +
          `<a href="#" id="empty-web-link">切到网页模式 ↗</a></div>`);
    messagesEl.appendChild(empty);
    document.getElementById("empty-web-link")?.addEventListener("click", (e) => {
      e.preventDefault();
      void enterWebMode();
    });
    return;
  }
  for (const m of store.messages) {
    if (m.role === "user") {
      appendUserMessage(m.content);
    } else {
      const refs = createAssistantMessage();
      refs.answerEl.textContent = m.content;
    }
  }
  scrollBottom();
}

function appendUserMessage(text: string) {
  const row = document.createElement("div");
  row.className = "msg user";
  const av = document.createElement("div");
  av.className = "avatar";
  av.textContent = "我";
  const b = document.createElement("div");
  b.className = "bubble";
  b.textContent = text;
  row.appendChild(av);
  row.appendChild(b);
  messagesEl.appendChild(row);
}

function createAssistantMessage(): { thinkingEl: HTMLDetailsElement; answerEl: HTMLElement } {
  const row = document.createElement("div");
  row.className = "msg assistant";
  const av = document.createElement("div");
  av.className = "avatar";
  av.textContent = "DS";
  const bubble = document.createElement("div");
  bubble.className = "bubble";
  const details = document.createElement("details");
  details.className = "thinking";
  details.hidden = true;
  const summary = document.createElement("summary");
  summary.textContent = "思考过程";
  const tc = document.createElement("div");
  tc.className = "thinking-content";
  details.appendChild(summary);
  details.appendChild(tc);
  const answer = document.createElement("div");
  answer.className = "answer";
  bubble.appendChild(details);
  bubble.appendChild(answer);
  row.appendChild(av);
  row.appendChild(bubble);
  messagesEl.appendChild(row);
  return { thinkingEl: details, answerEl: answer };
}

async function send() {
  const text = inputEl.value.trim();
  if (!text || sendBtn.disabled) return;
  if (!store.currentId) {
    const c = await api.createConversation();
    store.currentId = c.id;
    store.conversations.unshift(c);
    renderSidebar();
  }
  inputEl.value = "";
  autoGrow();
  appendUserMessage(text);
  const refs = createAssistantMessage();
  sendBtn.disabled = true;
  try {
    await api.chat(
      store.currentId,
      text,
      store.model,
      store.useMemory,
      store.thinking,
      (msg) => {
        const m = JSON.parse(msg) as { t: string; c: string };
        if (m.t === "reasoning") {
          refs.thinkingEl.hidden = false;
          refs.thinkingEl.open = true;
          refs.thinkingEl.querySelector(".thinking-content")!.textContent += m.c;
        } else {
          refs.answerEl.textContent += m.c;
        }
        scrollBottom();
      },
    );
    await store.refreshConversations();
    renderSidebar();
  } catch (e) {
    refs.answerEl.textContent += `\n[错误] ${e}`;
  } finally {
    sendBtn.disabled = false;
    inputEl.focus();
  }
}

function scrollBottom() {
  messagesEl.scrollTop = messagesEl.scrollHeight;
}

function autoGrow() {
  inputEl.style.height = "auto";
  inputEl.style.height = Math.min(inputEl.scrollHeight, 180) + "px";
}

/** 关闭设置弹窗；网页模式下恢复远程视图的显示。 */
function closeSettings() {
  settingsModal.hidden = true;
  document.body.classList.remove("modal-open");
  if (store.mode === "web") {
    void api.setSuppressed(false);
  }
}

function openSettings() {
  // 先显示弹窗（即便下面的 invoke 失败也不能让窗口打不开）
  document.body.classList.add("modal-open");
  settingsModal.hidden = false;
  if (store.mode === "web") api.setSuppressed(true).catch(() => {});
  apiKeyInput.value = "";
  apiKeyInput.focus();
  void showKeyStatus();
}

async function showKeyStatus() {
  const el = document.getElementById("key-status")!;
  try {
    const s = await api.apiKeyStatus();
    if (s.saved) {
      el.textContent = `当前已保存：${s.masked}　（${s.in_keyring ? "已存入系统钥匙串" : "本地加密文件存储"}）`;
    } else {
      el.textContent = "尚未保存 API Key。";
    }
  } catch {
    el.textContent = "";
  }
}

async function enterWebMode() {
  store.setMode("web");
  updateModeTabs();
  try {
    await api.openWebMode();
  } catch (e) {
    // 打开失败要回退模式，否则 app 会一直以为自己在网页模式（后续弹窗都去 setSuppressed）
    store.setMode("api");
    updateModeTabs();
    alert(`打开网页模式失败：${e}`);
  }
}

/** 侧边栏的网页/API 切换标签：网页模式激活右侧官方视图，API 模式隐藏它并回到本地聊天。 */
async function switchMode(mode: "web" | "api") {
  if (mode === "web") {
    const prev = store.mode;
    store.setMode("web");
    updateModeTabs();
    try {
      await api.openWebMode();
    } catch (e) {
      // 打开失败回退，避免卡在「半网页模式」
      store.setMode(prev || "api");
      updateModeTabs();
      alert(`打开网页模式失败：${e}`);
    }
  } else {
    store.setMode("api");
    updateModeTabs();
    await api.deactivateWebMode();
    if (!(await api.hasApiKey())) openSettings();
  }
}

/** 根据 store.mode 高亮侧边栏的网页/API 标签。 */
function updateModeTabs() {
  const web = document.getElementById("tab-web");
  const api = document.getElementById("tab-api");
  if (!web || !api) return;
  const m = store.mode;
  web.classList.toggle("active", m === "web");
  api.classList.toggle("active", m === "api");
}

/** 记忆有变动时，把最新快照推给已打开的网页模式窗口。失败无所谓，窗口没开而已。 */
function syncWeb() {
  api.syncWebMemories().catch(() => {});
}

async function openKeyPage() {
  try {
    await api.openUrl(DEEPSEEK_KEY_URL);
  } catch (e) {
    alert(`无法打开浏览器：${e}\n请手动访问 ${DEEPSEEK_KEY_URL}`);
  }
}

async function saveSettings() {
  const key = apiKeyInput.value.trim();
  // 空值表示清空 Key；非空则保存。Key 会持久化（钥匙串 / 本地加密文件），重启后仍记得。
  await api.setApiKey(key);
  apiKeyInput.value = "";
  closeSettings();
}

async function exportCurrent() {
  if (!store.currentId) return;
  const json = await api.exportConversation(store.currentId);
  const blob = new Blob([json], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  const c = store.conversations.find((x) => x.id === store.currentId) as Conversation;
  a.href = url;
  a.download = `${c ? c.title : "conversation"}.json`;
  a.click();
  URL.revokeObjectURL(url);
}

async function importFile(e: Event) {
  const file = (e.target as HTMLInputElement).files?.[0];
  if (!file) return;
  const text = await file.text();
  try {
    const conv = await api.importConversation(text);
    await store.refreshConversations();
    renderSidebar();
    await selectConversation(conv.id);
  } catch (err) {
    alert(`导入失败：${err}`);
  }
  (e.target as HTMLInputElement).value = "";
}

// ---------- 版本更新 ----------

/** 用户点了「跳过此版本」后记住版本号，同一个版本不再打扰 */
const SKIP_VERSION_KEY = "ds_skip_version";
/** 进程内一次性开关：启动检查每次开软件只跑一次 */
let startupChecked = false;

/**
 * 启动检查。设计原则是「只在真有新版本时才出现」：
 * 检查失败（断网、被墙、接口挂了）一律静默，绝不弹错误框骚扰用户。
 */
async function checkUpdateOnStartup() {
  if (startupChecked) return;
  startupChecked = true;
  let info: UpdateInfo;
  try {
    info = await api.checkUpdate();
  } catch {
    return;
  }
  if (!info.checked || !info.has_update) return;
  if (localStorage.getItem(SKIP_VERSION_KEY) === info.latest) return;
  showUpdate(info);
}

function showUpdate(info: UpdateInfo) {
  pendingUpdate = info;
  document.getElementById("update-cur")!.textContent = `v${info.current}`;
  document.getElementById("update-new")!.textContent = `v${info.latest}`;

  // GitHub 直连不通时，明确告诉用户「已经替你换成国内线路了」，别让人以为下载会失败
  const net = document.getElementById("update-net")!;
  net.hidden = info.github_reachable;
  if (!info.github_reachable) {
    net.textContent = "检测到当前网络无法直连 GitHub，已自动为你切换到国内加速镜像。";
  }

  // 更新说明：GitHub 的 body 是 Markdown，这里净化成纯文本展示（textContent 天然防注入）
  const notesWrap = document.getElementById("update-notes-wrap")!;
  const text = cleanNotes(info.notes);
  document.getElementById("update-notes")!.textContent = text;
  notesWrap.hidden = text.length === 0;

  // 备用镜像只在直连不通时列出，正常用户不需要看到这些
  const mirrors = document.getElementById("update-mirrors")!;
  const list = document.getElementById("update-mirror-list")!;
  list.innerHTML = "";
  const alt = info.mirror_urls.filter((u) => u !== info.download_url);
  if (!info.github_reachable && alt.length > 0) {
    for (const url of alt) {
      const b = document.createElement("button");
      b.className = "mirror-btn";
      b.textContent = mirrorName(url);
      b.addEventListener("click", () => void api.openUrl(url));
      list.appendChild(b);
    }
    mirrors.hidden = false;
  } else {
    mirrors.hidden = true;
  }

  // 网页模式下远程视图压在本地之上，弹窗前先把它藏起来，否则会被盖住。
  if (store.mode === "web") void api.setSuppressed(true);
  document.body.classList.add("modal-open");
  updateModal.hidden = false;
}

function closeUpdate() {
  updateModal.hidden = true;
  document.body.classList.remove("modal-open");
  if (store.mode === "web") void api.setSuppressed(false);
}

// ---------- 关于弹窗 ----------

function openAbout() {
  // 先显示弹窗（即便下面的 invoke 失败也不能让窗口打不开）
  document.body.classList.add("modal-open");
  aboutModal.hidden = false;
  if (store.mode === "web") api.setSuppressed(true).catch(() => {});
  // 展示当前版本号
  const label = document.getElementById("about-version-label");
  if (label) label.textContent = `版本 v${appVersion || "—"}`;
  // 清除上一次的检查结果，避免残留 banner
  aboutPendingUpdate = null;
  document.getElementById("about-update-banner")!.hidden = true;
  const st = document.getElementById("about-version-status");
  if (st) st.textContent = "";
}

function closeAbout() {
  aboutModal.hidden = true;
  document.body.classList.remove("modal-open");
  if (store.mode === "web") void api.setSuppressed(false);
}

/** 关于弹窗里的手动检查：与启动检查相反，无论结果如何都要给用户明确回执 */
async function manualCheckUpdateAbout() {
  const status = document.getElementById("about-version-status")!;
  const btn = document.getElementById("about-check-update-btn") as HTMLButtonElement;
  const label = document.getElementById("about-version-label")!;
  label.textContent = `版本 v${appVersion || "—"}`;
  btn.disabled = true;
  status.textContent = "检查中…";
  document.getElementById("about-update-banner")!.hidden = true;
  aboutPendingUpdate = null;
  try {
    const info = await api.checkUpdate();
    if (!info.checked) {
      status.textContent = "连不上更新服务器，请检查网络";
    } else if (info.has_update) {
      status.textContent = `发现新版本 v${info.latest}`;
      // 手动检查视为用户主动关心，之前「跳过此版本」的记录作废
      localStorage.removeItem(SKIP_VERSION_KEY);
      aboutPendingUpdate = info;
      document.getElementById("about-update-ver")!.textContent = `v${info.latest}`;
      document.getElementById("about-update-banner")!.hidden = false;
    } else {
      status.textContent = "已是最新版本";
    }
  } catch {
    status.textContent = "检查失败";
  } finally {
    btn.disabled = false;
  }
}

/** 把 Markdown 版发布说明压成弹窗能直接显示的纯文本 */
function cleanNotes(md: string): string {
  return md
    .replace(/!\[[^\]]*\]\([^)]*\)/g, "") // 图片整块去掉
    .replace(/\[([^\]]*)\]\([^)]*\)/g, "$1") // 链接只留文字
    .replace(/^#{1,6}\s*/gm, "") // 标题井号
    .replace(/^[-*]\s+/gm, "· ") // 列表符号换成中点
    .replace(/`{1,3}/g, "") // 代码反引号
    .replace(/\n{3,}/g, "\n\n")
    .trim()
    .slice(0, 1200);
}

/** 从镜像 URL 里取域名做按钮文案 */
function mirrorName(url: string): string {
  const m = /^https?:\/\/([^/]+)/.exec(url);
  return m ? `通过 ${m[1]} 下载 ↗` : "备用线路 ↗";
}

/** 仅开发预览用：造一份假更新信息，方便截图核对弹窗样式 */
function demoUpdateInfo(mirror: boolean): UpdateInfo {
  const official =
    "https://github.com/wolfprince12/DSonDT/releases/download/v0.3.4/DSonDT-0.3.4-aarch64.dmg";
  const mirrors = ["https://ghfast.top/", "https://gh-proxy.com/", "https://ghproxy.net/"].map(
    (m) => m + official,
  );
  return {
    checked: true,
    has_update: true,
    current: "0.3.4",
    latest: "0.3.5",
    github_reachable: !mirror,
    download_url: mirror ? mirrors[0]! : official,
    mirror_urls: mirrors,
    notes:
      "## 新增\n- 启动时自动检查版本更新\n- GitHub 不可达时自动引导到国内镜像下载\n\n## 修复\n- 修复设置页推广卡片在小窗口下的排版问题",
    release_url: "https://github.com/wolfprince12/DSonDT/releases/latest",
  };
}

// ---------- 记忆库 ----------

async function openMemory() {
  // 先显示弹窗（即便下面的 invoke 失败也不能让窗口打不开）
  document.body.classList.add("modal-open");
  memoryModal.hidden = false;
  if (store.mode === "web") api.setSuppressed(true).catch(() => {});
  await refreshMemories();
}

/** 关闭记忆库弹窗；网页模式下恢复远程视图的显示。 */
function closeMemory() {
  memoryModal.hidden = true;
  document.body.classList.remove("modal-open");
  if (store.mode === "web") {
    void api.setSuppressed(false);
  }
}

async function refreshMemories() {
  store.memories = await api.listMemories();
  renderMemories();
  syncWeb();
}

function renderMemories() {
  memoryListEl.innerHTML = "";
  const q = memorySearchEl.value.trim().toLowerCase();
  const items = store.memories.filter((m) => !q || m.content.toLowerCase().includes(q));
  if (items.length === 0) {
    const empty = document.createElement("div");
    empty.className = "memory-empty";
    empty.textContent = q ? "没有匹配的记忆" : "记忆库还是空的，点下方「新建记忆」添加一条吧";
    memoryListEl.appendChild(empty);
    return;
  }
  for (const m of items) {
    const item = document.createElement("div");
    item.className = "memory-item";
    item.dataset.id = String(m.id);

    const badge = document.createElement("span");
    badge.className = "memory-badge " + (m.origin === "manual" ? "manual" : "auto");
    badge.textContent = m.origin === "manual" ? "手动" : "自动";

    const content = document.createElement("div");
    content.className = "memory-content";
    content.textContent = m.content;

    const meta = document.createElement("div");
    meta.className = "memory-meta";
    meta.textContent = fmtDate(m.updated_at);

    const actions = document.createElement("div");
    actions.className = "memory-actions";
    const edit = document.createElement("button");
    edit.className = "ghost-btn";
    edit.textContent = "编辑";
    edit.addEventListener("click", () => startEditMemory(m));
    const del = document.createElement("button");
    del.className = "ghost-btn";
    del.textContent = "删除";
    del.addEventListener("click", () => void removeMemory(m.id));
    actions.appendChild(edit);
    actions.appendChild(del);

    item.appendChild(badge);
    item.appendChild(content);
    item.appendChild(meta);
    item.appendChild(actions);
    memoryListEl.appendChild(item);
  }
}

function startEditMemory(m: Memory) {
  const el = memoryListEl.querySelector(`[data-id="${m.id}"]`) as HTMLElement | null;
  if (!el) return;
  el.innerHTML = "";
  const ta = document.createElement("textarea");
  ta.className = "memory-edit-input";
  ta.rows = 3;
  ta.value = m.content;
  const actions = document.createElement("div");
  actions.className = "memory-actions";
  const save = document.createElement("button");
  save.className = "send-btn";
  save.style.cssText = "width:auto;padding:0 18px;height:36px;";
  save.textContent = "保存";
  save.addEventListener("click", () => void saveEditMemory(m.id, ta.value));
  const cancel = document.createElement("button");
  cancel.className = "ghost-btn";
  cancel.textContent = "取消";
  cancel.addEventListener("click", () => void refreshMemories());
  actions.appendChild(cancel);
  actions.appendChild(save);
  el.appendChild(ta);
  el.appendChild(actions);
  ta.focus();
}

async function saveEditMemory(id: number, content: string) {
  const c = content.trim();
  if (!c) return;
  await api.updateMemory(id, c);
  await refreshMemories();
}

async function removeMemory(id: number) {
  if (!confirm("确定删除这条记忆？")) return;
  await api.deleteMemory(id);
  await refreshMemories();
}

async function addMemory() {
  const c = memoryNewInput.value.trim();
  if (!c) return;
  await api.addMemory(c);
  memoryNewInput.value = "";
  memoryNewArea.hidden = true;
  await refreshMemories();
}
