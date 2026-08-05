//! # 单窗口双模式
//!
//! **硬性要求：网页模式与 API 模式必须共处同一个 OS 窗口**（铁律，绝不开第二个窗口）。
//! 两种平台「在同一窗口内如何呈现 DeepSeek」不同：
//!
//! - **macOS / Linux**：用 Tauri 2 `Window::add_child` 在主窗口之上叠加一个 `deepseek` 子 webview，
//!   占据整块区域。WKWebView 下 z-order 可控，叠加子视图不会吞掉主 webview 的点击。
//! - **Windows**：WebView2 的渲染面永远压在顶层（DirectX 合成，Tauri issue #6264），
//!   `add_child` 子视图会盖住整个父窗口、吞掉所有点击，完全不可用。
//!   因此 Windows 改为 **整页导航**：直接把主 webview 导航到 chat.deepseek.com，
//!   仍是同一个窗口、同一个 webview，只是页面在「本地 UI」与「DeepSeek」间切换。
//!   记忆注入脚本在 deepseek 域名下自动挂 🧠 / 💬 浮层；💬 返回即把主 webview 导航回本地首页。
//!   窗口控制交给 Windows 原生标题栏（decorations=true），保证任何模式下都能拖拽/最小化/关闭。

use std::sync::{Arc, Mutex};
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, PhysicalSize, Window};

pub const MAIN_WINDOW: &str = "main";
#[cfg_attr(target_os = "windows", allow(dead_code))]
pub const WEB_WEBVIEW: &str = "deepseek";

/// 必须与 style.css 里 `.app-topbar { height: 48px }` 保持一致。
/// （仅 macOS / Linux 的「同窗叠加子 webview」方案用得到；Windows 走主 webview 直接导航。）
#[cfg_attr(target_os = "windows", allow(dead_code))]
pub const TOP_BAR_H: f64 = 48.0;

/// 当前模式状态。`active` 表示是否在网页模式；`suppressed` 表示本地要弹模态，
/// 需要临时把网页视图藏起来（仅 macOS / Linux 的叠加子视图方案用到）。
#[derive(Default)]
pub struct WebStateInner {
    pub active: bool,
    #[cfg_attr(target_os = "windows", allow(dead_code))]
    pub suppressed: bool,
    /// 本地首页 URL（主 webview 初始加载的本地地址），Windows 网页模式返回时导航回它。
    pub home_url: String,
    /// Windows 专用：用户从 DeepSeek 点「💬 返回」后，Rust 把主 webview 导回本地首页并重载，
    /// 重载瞬间 store.mode 仍读 localStorage 的旧值 'web'，用这个标志纠正回 'api'。
    pub pending_api: bool,
}

#[derive(Clone, Default)]
pub struct WebState(pub Arc<Mutex<WebStateInner>>);

impl WebState {
    pub fn is_active(&self) -> bool {
        self.0.lock().unwrap().active
    }
    #[cfg_attr(target_os = "windows", allow(dead_code))]
    pub fn is_suppressed(&self) -> bool {
        self.0.lock().unwrap().suppressed
    }
    pub fn set_active(&self, v: bool) {
        self.0.lock().unwrap().active = v;
    }
    #[cfg_attr(target_os = "windows", allow(dead_code))]
    pub fn set_suppressed(&self, v: bool) {
        self.0.lock().unwrap().suppressed = v;
    }
    pub fn home_url(&self) -> String {
        self.0.lock().unwrap().home_url.clone()
    }
    pub fn set_home_url(&self, v: String) {
        self.0.lock().unwrap().home_url = v;
    }
    pub fn set_pending_api(&self, v: bool) {
        self.0.lock().unwrap().pending_api = v;
    }
    /// 读取并清空 pending_api（一次性消费，避免重复纠正）。
    pub fn take_pending_api(&self) -> bool {
        let mut g = self.0.lock().unwrap();
        let v = g.pending_api;
        g.pending_api = false;
        v
    }
}

/// 取主窗口逻辑像素尺寸（浮点，避免物理/逻辑来回转换的精度损失）。
pub fn window_size(window: &Window) -> (f64, f64) {
    let inner: PhysicalSize<u32> =
        window.inner_size().unwrap_or(PhysicalSize::new(1280, 820));
    let scale = window.scale_factor().unwrap_or(1.0);
    let w = inner.width as f64 / scale;
    let h = inner.height as f64 / scale;
    if w < 100.0 {
        (1280.0, 820.0)
    } else {
        (w, h)
    }
}

/// 布局调整：UI webview 永远铺满主窗口。
/// macOS：网页模式激活时让 deepseek 子视图覆盖整块区域（add_child 不会自动跟随父窗口缩放，需手动同步）。
/// Windows：整页导航，主 webview 自身即页面，无需额外摆位。
pub fn relayout(window: &Window) {
    let (w, h) = window_size(window);
    let app = window.app_handle();

    if let Some(ui) = app.get_webview(MAIN_WINDOW) {
        let _ = ui.set_position(LogicalPosition::new(0.0, 0.0));
        let _ = ui.set_size(LogicalSize::new(w, h));
    }

    #[cfg(not(target_os = "windows"))]
    if app.state::<WebState>().is_active() {
        if let Some(web) = app.get_webview(WEB_WEBVIEW) {
            let _ = web.set_position(LogicalPosition::new(0.0, TOP_BAR_H));
            let _ = web.set_size(LogicalSize::new(w, (h - TOP_BAR_H).max(1.0)));
        }
    }
}

/// 进入网页模式。
/// - Windows（整页导航）：直接把主 webview 导航到 chat.deepseek.com，并反复 eval 注入脚本
///   把 🧠 / 💬 浮层挂到 DeepSeek 页面上。
/// - macOS / Linux（叠加子 webview）：创建 deepseek 子视图并叠在本地 UI 之上。
#[cfg(target_os = "windows")]
pub fn activate(
    app: &AppHandle,
    memories_json: &str,
    home_url: &str,
    _platform: &str,
) -> Result<(), String> {
    let state = app.state::<WebState>();
    if state.is_active() {
        return Ok(());
    }

    if let Some(win) = app.get_webview_window(MAIN_WINDOW) {
        if let Ok(url) = "https://chat.deepseek.com".parse::<tauri::Url>() {
            let _ = win.navigate(url);
        }
    }
    state.set_active(true);
    state.set_suppressed(false);

    // 主 webview 导航到 DeepSeek 是异步的，且 DeepSeek 是 SPA（先白屏再渲染）。
    // 反复 eval 注入脚本，前几次若还没切到 deepseek 域名会被 self-guard 跳过，
    // 一旦切过去就挂载浮层；即便中途被页面重载冲掉也会重新挂上。
    let app2 = app.clone();
    let script = inject_script(memories_json, home_url, "win");
    std::thread::spawn(move || {
        for _ in 0..14 {
            if let Some(w) = app2.get_webview_window(MAIN_WINDOW) {
                let _ = w.eval(&script);
            }
            std::thread::sleep(std::time::Duration::from_millis(800));
        }
    });

    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn activate(
    app: &AppHandle,
    memories_json: &str,
    _home_url: &str,
    _platform: &str,
) -> Result<(), String> {
    let state = app.state::<WebState>();
    if state.is_active() {
        if state.is_suppressed() {
            set_suppressed(app, false);
        }
        return Ok(());
    }

    let main = app
        .get_window(MAIN_WINDOW)
        .ok_or_else(|| "主窗口不存在".to_string())?;
    let (w, h) = window_size(&main);

    let url: tauri::Url = "https://chat.deepseek.com"
        .parse()
        .expect("deepseek URL 必合法");
    let app_for_nav = app.clone();
    main.add_child(
        tauri::WebviewBuilder::new(WEB_WEBVIEW, tauri::WebviewUrl::External(url))
            .initialization_script(&inject_script(memories_json, "", "mac"))
            .on_navigation(move |nav_url| {
                // deepseek webview 通过 location.replace('dsondt://xxx') 通知宿主；
                // macOS(WKWebView) / Windows(WebView2) 都会先触发导航决策回调，
                // 我们返回 false 阻止实际导航（在 delegate/浏览器引擎决定前
                // 不会真去解析 dsondt: scheme，也不会影响主页面 URL）。
                if nav_url.scheme() == "dsondt" {
                    let action = nav_url.host_str().unwrap_or("").to_string();
                    let app = app_for_nav.clone();
                    tauri::async_runtime::spawn(async move {
                        handle_dsondt_action(&app, &action);
                    });
                    return false;
                }
                true
            }),
        LogicalPosition::new(0.0, TOP_BAR_H),
        LogicalSize::new(w, (h - TOP_BAR_H).max(1.0)),
    )
    .map_err(|e| format!("创建 deepseek 子视图失败：{e}"))?;

    state.set_active(true);
    state.set_suppressed(false);

    if let Some(main_win) = app.get_window(MAIN_WINDOW) {
        relayout(&main_win);
    }

    Ok(())
}

/// 处理 deepseek webview 通过 `dsondt://<action>` 派发过来的动作。
/// 当前支持的 action：
/// - `open-memory`：藏起远程视图，通知本地 UI 弹出记忆库面板
fn handle_dsondt_action(app: &AppHandle, action: &str) {
    match action {
        "open-memory" => {
            set_suppressed(app, true);
            if let Some(ui) = app.get_webview(MAIN_WINDOW) {
                let _ = ui.eval("window.dispatchEvent(new CustomEvent('dsondt:open-memory'))");
            }
        }
        _ => {
            // 未知 action：忽略，避免误处理未来扩展时的新 scheme
        }
    }
}

/// 退出网页模式。
/// - Windows（整页导航）：把主 webview 导回本地首页，并置 pending_api 让前端纠正回 API 模式。
/// - macOS / Linux（叠加子 webview）：关闭 deepseek 子视图，下次切回重新 add_child。
#[cfg(target_os = "windows")]
pub fn deactivate(app: &AppHandle) {
    let state = app.state::<WebState>();
    let home = state.home_url();
    state.set_active(false);
    state.set_suppressed(false);
    state.set_pending_api(true);
    if let Some(win) = app.get_webview_window(MAIN_WINDOW) {
        if let Ok(url) = home.parse::<tauri::Url>() {
            let _ = win.navigate(url);
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn deactivate(app: &AppHandle) {
    let state = app.state::<WebState>();
    state.set_active(false);
    state.set_suppressed(false);
    state.set_pending_api(false);
    if let Some(w) = app.get_webview(WEB_WEBVIEW) {
        let _ = w.close();
    }
}

pub fn is_active(app: &AppHandle) -> bool {
    app.state::<WebState>().is_active()
}

/// 读取并清空 pending_api 标志（命令层包装）。
pub fn take_pending_api(app: &AppHandle) -> bool {
    app.state::<WebState>().take_pending_api()
}

/// 临时隐藏 / 恢复 deepseek 视图（用于本地弹模态）。
/// macOS / Linux：直接 hide/show 子 webview（WKWebView 上可靠）；
/// Windows：整页导航下没有独立子视图，本地弹窗时本来就在本地 UI 上，无需处理。
pub fn set_suppressed(app: &AppHandle, suppressed: bool) {
    let state = app.state::<WebState>();
    state.set_suppressed(suppressed);
    #[cfg(not(target_os = "windows"))]
    {
        if !state.is_active() {
            return;
        }
        if let Some(w) = app.get_webview(WEB_WEBVIEW) {
            if suppressed {
                let _ = w.hide();
            } else {
                let _ = w.show();
            }
        }
    }
    #[cfg(target_os = "windows")]
    {
        let _ = suppressed;
    }
}

/// 把最新记忆 JSON 推给当前正在显示的 deepseek 视图。
/// - macOS / Linux：子 webview。
/// - Windows：整页导航下，deepseek 页面就是主 webview 本身。
#[cfg(target_os = "windows")]
pub fn push_memories(app: &AppHandle, json: &str) {
    let js = format!(
        "try{{window.__DSONDT__&&window.__DSONDT__.setMemories({json})}}catch(e){{}}"
    );
    if let Some(w) = app.get_webview_window(MAIN_WINDOW) {
        let _ = w.eval(&js);
    }
}

#[cfg(not(target_os = "windows"))]
pub fn push_memories(app: &AppHandle, json: &str) {
    let js = format!(
        "try{{window.__DSONDT__&&window.__DSONDT__.setMemories({json})}}catch(e){{}}"
    );
    if let Some(w) = app.get_webview(WEB_WEBVIEW) {
        let _ = w.eval(&js);
    }
}

/// 注入到 deepseek 页面的 JS：
/// - 🧠 注入记忆：把本地记忆按相关性拼成前缀块、兼容 React 受控组件地填进输入框（用户自己按回车）；
/// - 💬 返回 API：Windows 直接把主 webview 导航回本地首页；macOS 调 deactivate_web_mode；
/// - 被动监听 fetch 把用户消息沉淀为 web 记忆；IPC 不可用时本地二元组打分兜底。
/// 记忆快照在初始化时烘焙进脚本（`__DSONDT_MEMORIES__` 占位符由 `inject_script` 替换）。
/// 主页 URL / 平台标记分别由 `__DSONDT_HOME__` / `__DSONDT_PLATFORM__` 占位符替换。
const INJECT_JS: &str = r##"
(function () {
  if (window.__DSONDT_INJECTED__) return;
  window.__DSONDT_INJECTED__ = true;

  // 只在 DeepSeek 页面挂载；本地 UI 页直接跳过，避免误挂按钮。
  if (location.hostname.indexOf('deepseek') === -1) return;

  var MEMORIES = __DSONDT_MEMORIES__;
  var HOME_URL = '__DSONDT_HOME__';
  var IS_WIN = '__DSONDT_PLATFORM__' === 'win';
  var TOP_K = 5;
  var autoSink = true;

  function invoke(cmd, args) {
    try {
      var it = window.__TAURI_INTERNALS__;
      if (it && typeof it.invoke === 'function') return it.invoke(cmd, args || {});
    } catch (e) {}
    return Promise.reject(new Error('ipc-unavailable'));
  }

  function bigrams(s) {
    s = (s || '').toLowerCase().replace(/\s+/g, '');
    var out = [];
    if (s.length < 2) return s ? [s] : [];
    for (var i = 0; i < s.length - 1; i++) out.push(s.slice(i, i + 2));
    return out;
  }
  function localSearch(q, k) {
    if (!MEMORIES || !MEMORIES.length) return [];
    var qg = bigrams(q);
    if (!qg.length) {
      return MEMORIES.slice(0, k).map(function (m) { return m.content; });
    }
    var scored = [];
    for (var i = 0; i < MEMORIES.length; i++) {
      var cg = bigrams(MEMORIES[i].content);
      if (!cg.length) continue;
      var set = Object.create(null);
      for (var j = 0; j < cg.length; j++) set[cg[j]] = 1;
      var hit = 0;
      for (var n = 0; n < qg.length; n++) if (set[qg[n]]) hit++;
      var s = hit / qg.length;
      if (s > 0.05) scored.push({ s: s, c: MEMORIES[i].content });
    }
    scored.sort(function (a, b) { return b.s - a.s; });
    return scored.slice(0, k).map(function (x) { return x.c; });
  }

  function search(q) {
    return invoke('search_memories', { query: q, topK: TOP_K })
      .then(function (r) { return (r && r.length) ? r : localSearch(q, TOP_K); })
      .catch(function () { return localSearch(q, TOP_K); });
  }

  function findInput() {
    var sels = [
      'textarea#chat-input',
      'textarea[id*="chat"]',
      'div[contenteditable="true"][role="textbox"]',
      'textarea',
      'div[contenteditable="true"]'
    ];
    for (var i = 0; i < sels.length; i++) {
      var list = document.querySelectorAll(sels[i]);
      for (var j = 0; j < list.length; j++) {
        var el = list[j];
        var r = el.getBoundingClientRect();
        if (r.width > 120 && r.height > 12) return el;
      }
    }
    return null;
  }

  function readInput(el) {
    if (!el) return '';
    if (el.tagName === 'TEXTAREA' || el.tagName === 'INPUT') return el.value || '';
    return el.innerText || '';
  }

  function writeInput(el, val) {
    if (el.tagName === 'TEXTAREA' || el.tagName === 'INPUT') {
      var proto = el.tagName === 'TEXTAREA' ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
      var desc = Object.getOwnPropertyDescriptor(proto, 'value');
      if (desc && desc.set) desc.set.call(el, val); else el.value = val;
      el.dispatchEvent(new Event('input', { bubbles: true }));
      el.focus();
      try { el.setSelectionRange(val.length, val.length); } catch (e) {}
      return true;
    }
    try {
      el.focus();
      var sel = window.getSelection();
      var range = document.createRange();
      range.selectNodeContents(el);
      sel.removeAllRanges();
      sel.addRange(range);
      document.execCommand('insertText', false, val);
      return true;
    } catch (e) { return false; }
  }

  function buildBlock(mems) {
    var lines = ['【长期记忆 · 由 DSonDT 本地记忆库提供，请在本轮回答中自然参考，不要复述本段】'];
    for (var i = 0; i < mems.length; i++) lines.push((i + 1) + '. ' + mems[i]);
    lines.push('【记忆结束，以下是我的问题】');
    return lines.join('\n') + '\n\n';
  }

  var host = document.createElement('div');
  host.id = '__dsondt_host';
  host.style.cssText = 'position:fixed;right:18px;bottom:96px;z-index:2147483647;';
  var shadow = host.attachShadow({ mode: 'open' });
  shadow.innerHTML = [
    '<style>',
    ':host{all:initial}',
    '*{box-sizing:border-box;font-family:-apple-system,BlinkMacSystemFont,"Segoe UI","PingFang SC","Microsoft YaHei",sans-serif}',
    '.wrap{display:flex;flex-direction:column;align-items:flex-end;gap:8px}',
    '.btn{display:flex;align-items:center;gap:6px;height:38px;padding:0 14px;border:none;border-radius:19px;',
    'background:#4d6bfe;color:#fff;font-size:13px;font-weight:500;cursor:pointer;',
    'box-shadow:0 4px 16px rgba(77,107,254,.35);transition:transform .15s,box-shadow .15s}',
    '.btn:hover{transform:translateY(-1px);box-shadow:0 6px 20px rgba(77,107,254,.45)}',
    '.btn:active{transform:translateY(0)}',
    '.btn.sec{background:rgba(120,120,140,.18);color:#5a5a68;box-shadow:none;height:30px;font-size:12px;padding:0 11px}',
    '.badge{min-width:18px;height:18px;padding:0 5px;border-radius:9px;background:rgba(255,255,255,.28);',
    'font-size:11px;line-height:18px;text-align:center}',
    '.toast{max-width:280px;padding:9px 13px;border-radius:9px;background:rgba(28,28,32,.92);color:#fff;',
    'font-size:12px;line-height:1.5;opacity:0;transform:translateY(6px);transition:opacity .2s,transform .2s;pointer-events:none}',
    '.toast.show{opacity:1;transform:translateY(0)}',
    '</style>',
    '<div class="wrap">',
    '<div class="toast" id="t"></div>',
    // 「🧠 注入记忆」：把本地记忆相关片段前置填进 DeepSeek 输入框。
    '<button class="btn" id="inj">🧠 注入记忆 <span class="badge" id="b">0</span></button>',
    // 「💬 返回 API」：Windows 把主 webview 导回本地首页（单窗口切换，不另开窗口）；
    // macOS 等价于点顶栏标签切回。
    '<button class="btn sec" id="ret">💬 返回 API</button>',
    '</div>'
  ].join('');

  function mount() {
    if (!document.body) return setTimeout(mount, 300);
    if (!document.getElementById('__dsondt_host')) document.body.appendChild(host);
    updateBadge();
  }

  var toastTimer = null;
  function toast(msg) {
    var t = shadow.getElementById('t');
    t.textContent = msg;
    t.classList.add('show');
    clearTimeout(toastTimer);
    toastTimer = setTimeout(function () { t.classList.remove('show'); }, 2600);
  }
  function updateBadge() {
    var b = shadow.getElementById('b');
    if (b) b.textContent = String((MEMORIES && MEMORIES.length) || 0);
  }

  function doInject() {
    var el = findInput();
    var cur = readInput(el).trim();
    if (cur.indexOf('【长期记忆') === 0) { toast('本条消息已经注入过记忆了'); return; }
    search(cur).then(function (mems) {
      if (!mems || !mems.length) {
        toast('没有匹配到相关记忆。先在记忆库里加几条吧。');
        return;
      }
      var block = buildBlock(mems);
      if (el && writeInput(el, block + cur)) {
        toast('已注入 ' + mems.length + ' 条记忆，检查无误后按回车发送');
      } else {
        var full = block + cur;
        if (navigator.clipboard) {
          navigator.clipboard.writeText(full).then(function () {
            toast('没找到输入框，内容已复制到剪贴板，请手动粘贴');
          }).catch(function () { toast('注入失败，且无法访问剪贴板'); });
        } else {
          toast('没找到输入框，注入失败');
        }
      }
    });
  }

  shadow.getElementById('inj').addEventListener('click', doInject);

  shadow.getElementById('ret').addEventListener('click', function () {
    if (IS_WIN) {
      // 直接把主 webview 导回本地首页，并带 #return-api 标记让本地 UI 纠正回 API 模式
      // （不依赖 IPC：在 DeepSeek 这种外部页面上，纯导航最稳）。
      window.location.href = HOME_URL + '#return-api';
    } else {
      invoke('deactivate_web_mode').catch(function () {});
    }
  });

  document.addEventListener('keydown', function (e) {
    if ((e.metaKey || e.ctrlKey) && (e.key === 'm' || e.key === 'M')) {
      e.preventDefault();
      doInject();
    }
  });

  var origFetch = window.fetch;
  window.fetch = function (input, init) {
    try {
      if (autoSink && init && init.body && typeof init.body === 'string') {
        var url = typeof input === 'string' ? input : (input && input.url) || '';
        if (url.indexOf('/completion') !== -1 || url.indexOf('/chat/') !== -1) {
          var body = JSON.parse(init.body);
          var text = body.prompt || body.message || body.content || '';
          if (typeof text === 'string') {
            var idx = text.indexOf('【记忆结束，以下是我的问题】');
            if (idx !== -1) text = text.slice(idx + '【记忆结束，以下是我的问题】'.length);
            text = text.trim();
            if (text.length >= 4 && text.length <= 2000) {
              invoke('add_web_memory', { content: text }).catch(function () {});
            }
          }
        }
      }
    } catch (e) {}
    return origFetch.apply(this, arguments);
  };

  window.__DSONDT__ = {
    setMemories: function (list) { MEMORIES = list || []; updateBadge(); },
    setAutoSink: function (v) { autoSink = !!v; },
    inject: doInject
  };

  // 兜底：万一 DeepSeek 页面在内置浏览器里彻底渲染不出来（网络不通 / WebView2 太旧 /
  // SPA 报错），别把用户丢在一片纯白里 —— 给一块可操作的说明面板 + 诊断信息。
  function bailout() {
    try {
      if (document.getElementById('__dsondt_fallback')) return;
      var txt = (document.body && document.body.innerText ? document.body.innerText : '').trim();
      if (txt.length > 20) return;
      if (document.querySelector('textarea')) return;
      if (document.querySelector('[contenteditable="true"]')) return;
      var d = document.createElement('div');
      d.id = '__dsondt_fallback';
      d.style.cssText = 'position:fixed;left:0;top:0;right:0;bottom:0;z-index:2147483646;background:#fff;' +
        'color:#1c1c22;display:flex;align-items:center;justify-content:center;padding:32px;' +
        'font:14px/1.7 -apple-system,BlinkMacSystemFont,"Segoe UI","PingFang SC","Microsoft YaHei",sans-serif';
      d.innerHTML = [
        '<div style="max-width:560px">',
        '<div style="font-size:18px;font-weight:600;margin-bottom:10px">DeepSeek 网页没能加载出来</div>',
        '<div style="color:#5a5a68;margin-bottom:18px">页面在内置浏览器里一直是空白。常见原因：网络不通或需要代理；系统的 WebView2 运行时版本过旧。可以先点「重新加载」试一次。</div>',
        '<div style="display:flex;gap:10px;flex-wrap:wrap;margin-bottom:16px">',
        '<button id="__ds_rl" style="height:36px;padding:0 16px;border:none;border-radius:8px;background:#4d6bfe;color:#fff;font-size:13px;cursor:pointer">重新加载</button>',
        '<button id="__ds_cp" style="height:36px;padding:0 16px;border:1px solid #d5d5de;border-radius:8px;background:#fff;color:#1c1c22;font-size:13px;cursor:pointer">复制诊断信息</button>',
        '</div>',
        '<pre id="__ds_ua" style="white-space:pre-wrap;word-break:break-all;background:#f4f4f7;border-radius:8px;padding:12px;font-size:11px;color:#5a5a68;margin:0"></pre>',
        '</div>'
      ].join('');
      (document.body || document.documentElement).appendChild(d);
      var info = 'URL: ' + location.href + '\nUA: ' + navigator.userAgent +
        '\nonLine: ' + navigator.onLine + '\nsize: ' + window.innerWidth + 'x' + window.innerHeight +
        '\ndpr: ' + window.devicePixelRatio;
      d.querySelector('#__ds_ua').textContent = info;
      d.querySelector('#__ds_rl').addEventListener('click', function () { location.reload(); });
      d.querySelector('#__ds_cp').addEventListener('click', function () {
        try { navigator.clipboard.writeText(info); } catch (e) {}
      });
    } catch (e) {}
  }
  setTimeout(bailout, 9000);

  mount();
  setInterval(function () {
    if (document.body && !document.getElementById('__dsondt_host')) document.body.appendChild(host);
  }, 2000);
})();
"##;

/// 烘焙记忆快照 / 首页 URL / 平台标记：把占位符替换为实际值。
pub fn inject_script(memories_json: &str, home_url: &str, platform: &str) -> String {
    INJECT_JS
        .replace("__DSONDT_MEMORIES__", memories_json)
        .replace("__DSONDT_HOME__", home_url)
        .replace("__DSONDT_PLATFORM__", platform)
}
