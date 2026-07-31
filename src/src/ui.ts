import { api } from "./api";
import { store } from "./store";
import type { Conversation, Memory } from "./types";

let convListEl: HTMLElement;
let messagesEl: HTMLElement;
let inputEl: HTMLTextAreaElement;
let modelSelect: HTMLSelectElement;
let memoryToggle: HTMLInputElement;
let thinkToggle: HTMLInputElement;
let settingsModal: HTMLElement;
let apiKeyInput: HTMLInputElement;
let themeSelect: HTMLSelectElement;
let sendBtn: HTMLButtonElement;
let memoryModal: HTMLElement;
let memoryListEl: HTMLElement;
let memorySearchEl: HTMLInputElement;
let memoryNewArea: HTMLElement;
let memoryNewInput: HTMLTextAreaElement;

const MODELS = [
  { id: "deepseek-v4-flash", name: "DeepSeek V4 Flash" },
  { id: "deepseek-v4-pro", name: "DeepSeek V4 Pro" },
];

const DEEPSEEK_KEY_URL = "https://platform.deepseek.com/api_keys";

export async function initUI() {
  const app = document.getElementById("app")!;
  app.innerHTML = template();
  // 平台标识：仅 macOS 需要在顶栏左侧留 80px 让位给浮动的红黄绿按钮
  // （Rust 侧用 titleBarStyle=Overlay 隐藏了原生标题栏，按钮会浮在我们顶栏之上）。
  if (typeof navigator !== "undefined" && /mac/i.test(navigator.platform || "")) {
    document.body.classList.add("platform-mac");
  }
  convListEl = document.getElementById("conv-list")!;
  messagesEl = document.getElementById("messages")!;
  inputEl = document.getElementById("input") as HTMLTextAreaElement;
  modelSelect = document.getElementById("model-select") as HTMLSelectElement;
  memoryToggle = document.getElementById("memory-toggle") as HTMLInputElement;
  thinkToggle = document.getElementById("think-toggle") as HTMLInputElement;
  settingsModal = document.getElementById("settings-modal")!;
  apiKeyInput = document.getElementById("api-key-input") as HTMLInputElement;
  themeSelect = document.getElementById("theme-select") as HTMLSelectElement;
  sendBtn = document.getElementById("send-btn") as HTMLButtonElement;
  memoryModal = document.getElementById("memory-modal")!;
  memoryListEl = document.getElementById("memory-list")!;
  memorySearchEl = document.getElementById("memory-search") as HTMLInputElement;
  memoryNewArea = document.getElementById("memory-new")!;
  memoryNewInput = document.getElementById("memory-new-input") as HTMLTextAreaElement;

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
  // 顶栏按钮：记忆库 / 设置
  document.getElementById("memory-btn")!.addEventListener("click", () => void openMemory());
  document.getElementById("settings-btn")!.addEventListener("click", () => openSettings());
  // 网页模式里注入的「📚 编辑记忆库」按钮会经 Rust 派发这个事件，由本地 UI 打开弹窗
  window.addEventListener("dsondt:open-memory", () => void openMemory());
  // Esc 关闭当前打开的模态框
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      if (!settingsModal.hidden) closeSettings();
      else if (!memoryModal.hidden) closeMemory();
    }
  });
  document.getElementById("settings-cancel")!.addEventListener("click", () => closeSettings());
  document.getElementById("settings-save")!.addEventListener("click", saveSettings);
  document.getElementById("get-key-btn")!.addEventListener("click", () => void openKeyPage());
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
  } else if (store.mode === "api" && !(await api.hasApiKey())) {
    openSettings();
  }
  updateModeTabs();
  await store.refreshConversations();
  renderSidebar();
  if (store.conversations.length > 0) {
    await selectConversation(store.conversations[0].id);
  } else {
    newChat();
  }
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
    <div class="modal">
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
  // 网页模式下远程视图压在本地之上，弹窗前先把它藏起来，否则会被盖住。
  if (store.mode === "web") {
    void api.setSuppressed(true);
  }
  document.body.classList.add("modal-open");
  settingsModal.hidden = false;
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
    alert(`打开网页模式失败：${e}`);
  }
}

/** 侧边栏的网页/API 切换标签：网页模式激活右侧官方视图，API 模式隐藏它并回到本地聊天。 */
async function switchMode(mode: "web" | "api") {
  if (mode === "web") {
    store.setMode("web");
    updateModeTabs();
    try {
      await api.openWebMode();
    } catch (e) {
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

// ---------- 记忆库 ----------

async function openMemory() {
  // 网页模式下远程视图压在本地之上，弹窗前先把它藏起来，否则会被盖住。
  if (store.mode === "web") {
    await api.setSuppressed(true);
  }
  document.body.classList.add("modal-open");
  memoryModal.hidden = false;
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
