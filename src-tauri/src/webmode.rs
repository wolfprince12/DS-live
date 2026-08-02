//! # 单窗口双模式（macOS / Windows 通用）
//!
//! API 模式 = 主窗口的「主 webview」即本地 UI（铺满，含顶栏 + 侧栏 + 聊天区）。
//! 网页模式 = 在主窗口之上叠出 `deepseek` 这个子 webview，占据顶栏下方的整幅区域。
//!
//! 用 Tauri 2 `unstable` 的 `Window::add_child` 把 deepseek 叠加到主 webview 之上，二者共处一个窗口
//! （macOS 下是同一个 NSWindow；Windows 下是同一个父窗口 HWND 上的 WebView2 子视图），行为一致。
//! 顶栏由主 webview 顶部 48px 自己画，`deepseek` webview 用 `LogicalPosition(0, 48)` 摆在它下面。
//!
//! 重要：`-webkit-app-region: drag` 只在「主 webview」上生效，子 webview 上会被系统无视；
//! 因此本地 UI 必须是主 webview，deepseek 才是叠加的子视图（这是鼠标拖动能生效的前提）。
//! 拖动统一通过 JS `getCurrentWindow().startDragging()` 触发，对 macOS / Windows 均可靠。

use std::sync::{Arc, Mutex};
use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, PhysicalSize, WebviewBuilder, WebviewUrl,
    Window,
};

pub const MAIN_WINDOW: &str = "main";
pub const WEB_WEBVIEW: &str = "deepseek";

/// 必须与 style.css 里 `.app-topbar { height: 48px }` 保持一致。
pub const TOP_BAR_H: f64 = 48.0;

/// 当前模式状态。`active` 表示是否在网页模式；`suppressed` 表示本地要弹模态，
/// 需要临时把网页视图藏起来。
/// `last_memories` 缓存最近一次进入网页模式时的记忆 JSON，供 modal 关闭后重建子视图使用
/// （避免 WebView2 上 hide()/show() 不可靠时 deepseek 仍遮在主 webview 之上挡 modal）。
#[derive(Default)]
pub struct WebStateInner {
    pub active: bool,
    pub suppressed: bool,
    pub last_memories: String,
}

#[derive(Clone, Default)]
pub struct WebState(pub Arc<Mutex<WebStateInner>>);

impl WebState {
    pub fn is_active(&self) -> bool {
        self.0.lock().unwrap().active
    }
    pub fn is_suppressed(&self) -> bool {
        self.0.lock().unwrap().suppressed
    }
    pub fn set_active(&self, v: bool) {
        self.0.lock().unwrap().active = v;
    }
    pub fn set_suppressed(&self, v: bool) {
        self.0.lock().unwrap().suppressed = v;
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

/// 布局调整：UI webview 永远铺满主窗口；网页模式激活时再让 deepseek 子视图
/// 覆盖顶栏下方整块区域（add_child 不会自动跟随父窗口缩放，需手动同步）。
pub fn relayout(window: &Window) {
    let (w, h) = window_size(window);
    let app = window.app_handle();

    // 本地 UI（主 webview）永远铺满整个窗口（含顶栏）
    if let Some(ui) = app.get_webview(MAIN_WINDOW) {
        let _ = ui.set_position(LogicalPosition::new(0.0, 0.0));
        let _ = ui.set_size(LogicalSize::new(w, h));
    }

    if !app.state::<WebState>().is_active() {
        return;
    }

    // Windows 上网页模式是独立窗口，不由 relayout 管理；仅 macOS / Linux 需要同步子 webview 位置。
    #[cfg(not(target_os = "windows"))]
    if app.state::<WebState>().is_active() {
        if let Some(web) = app.get_webview(WEB_WEBVIEW) {
            let _ = web.set_position(LogicalPosition::new(0.0, TOP_BAR_H));
            let _ = web.set_size(LogicalSize::new(w, (h - TOP_BAR_H).max(1.0)));
        }
    }
}

/// 进入网页模式：创建 deepseek 子视图，摆位 + 注入记忆。
/// 通过 `on_navigation` 拦截 `dsondt://` 自定义 scheme 来桥接外部 webview
/// 与宿主本地命令——不走 Tauri IPC（外部 URL 的 webview 调自定义命令在 Tauri 2
/// 已知有 capability 坑，issue #10298/#10317），更稳。
#[cfg(not(target_os = "windows"))]
pub fn activate(app: &AppHandle, memories_json: &str) -> Result<(), String> {
    let state = app.state::<WebState>();
    // 缓存记忆 JSON，供 modal 关闭后重建子视图用
    state.0.lock().unwrap().last_memories = memories_json.to_string();
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
        WebviewBuilder::new(WEB_WEBVIEW, WebviewUrl::External(url))
            .initialization_script(&inject_script(memories_json))
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

    // 关键：Tauri 2 的 `WebviewBuilder::auto_resize()` 在部分平台（如 macOS）上会把刚 add_child 的
    // 子 webview 强制拉回 (0, 0) 铺满父窗口，导致覆盖 DSonDT 顶栏的 (0..48) 区域。
    // 这里**不使用** auto_resize，改由 `main.rs` 的 `Resized` listener + 本文件的
    // `relayout()` 手动同步位置/尺寸。add_child 之后主动再摆一次，吸收任何初始化
    // 阶段被覆盖的位置。
    if let Some(main_win) = app.get_window(MAIN_WINDOW) {
        relayout(&main_win);
    }

    Ok(())
}

/// 网页模式（Windows）：**不用 add_child**（WebView2 上 z-order 不可控、渲染表面压顶吞点击，
/// 见 Tauri issue #6264 与 WebView2 的「Topmost Render Surface」问题），改用独立的无边框
/// WebviewWindow 加载 chat.deepseek.com，并排在主窗口右侧。deepseek 是普通 OS 窗口，
/// 绝不覆盖主 webview，主窗口所有按钮永远可点；modal 打开时 hide 此窗口、关闭后 show。
#[cfg(target_os = "windows")]
pub fn activate(app: &AppHandle, memories_json: &str) -> Result<(), String> {
    let state = app.state::<WebState>();
    state.0.lock().unwrap().last_memories = memories_json.to_string();
    if state.is_active() {
        push_memories(app, memories_json);
        return Ok(());
    }
    let url: tauri::Url = "https://chat.deepseek.com".parse().expect("deepseek URL 必合法");
    let (w, h, x, y) = if let Some(main) = app.get_window(MAIN_WINDOW) {
        let p = main
            .outer_position()
            .unwrap_or(tauri::PhysicalPosition::new(0, 0));
        let s = main
            .outer_size()
            .unwrap_or(tauri::PhysicalSize::new(1280, 820));
        (s.width as f64, s.height as f64, p.x as f64, p.y as f64)
    } else {
        (1280.0, 820.0, 80.0, 80.0)
    };
    tauri::WebviewWindowBuilder::new(app, WEB_WEBVIEW, WebviewUrl::External(url))
        .title("DSonDT · DeepSeek")
        .decorations(false)
        .inner_size(w, h)
        .position(x + w + 8.0, y)
        .initialization_script(&inject_script(memories_json))
        .build()
        .map_err(|e| format!("创建 deepseek 窗口失败：{e}"))?;
    state.set_active(true);
    state.set_suppressed(false);
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

/// 退出网页模式：deepseek 子视图关闭，下次切回重新 add_child。
#[cfg(not(target_os = "windows"))]
pub fn deactivate(app: &AppHandle) {
    let state = app.state::<WebState>();
    state.set_active(false);
    state.set_suppressed(false);
    if let Some(w) = app.get_webview(WEB_WEBVIEW) {
        let _ = w.close();
    }
}

#[cfg(target_os = "windows")]
pub fn deactivate(app: &AppHandle) {
    let state = app.state::<WebState>();
    state.set_active(false);
    state.set_suppressed(false);
    if let Some(w) = app.get_webview_window(WEB_WEBVIEW) {
        let _ = w.close();
    }
}

pub fn is_active(app: &AppHandle) -> bool {
    app.state::<WebState>().is_active()
}

/// 临时隐藏 / 恢复 deepseek 视图（用于本地弹模态）。
/// macOS / Linux：直接 hide/show 子 webview（WKWebView 上可靠）；
/// Windows：hide/show 独立窗口（OS 级操作，比 add_child 子 webview 的 hide 可靠得多）。
#[cfg(not(target_os = "windows"))]
pub fn set_suppressed(app: &AppHandle, suppressed: bool) {
    let state = app.state::<WebState>();
    state.set_suppressed(suppressed);
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
pub fn set_suppressed(app: &AppHandle, suppressed: bool) {
    let state = app.state::<WebState>();
    state.set_suppressed(suppressed);
    if let Some(w) = app.get_webview_window(WEB_WEBVIEW) {
        if suppressed {
            let _ = w.hide();
        } else {
            let _ = w.show();
            let _ = w.set_focus();
        }
    }
}

/// 把最新记忆 JSON 推给当前正在显示的 deepseek 视图（macOS / Linux：子 webview）。
#[cfg(not(target_os = "windows"))]
pub fn push_memories(app: &AppHandle, json: &str) {
    let js = format!(
        "try{{window.__DSONDT__&&window.__DSONDT__.setMemories({json})}}catch(e){{}}"
    );
    if let Some(w) = app.get_webview(WEB_WEBVIEW) {
        let _ = w.eval(&js);
    }
}

/// 把最新记忆 JSON 推给当前正在显示的 deepseek 视图（Windows：独立窗口）。
#[cfg(target_os = "windows")]
pub fn push_memories(app: &AppHandle, json: &str) {
    let js = format!(
        "try{{window.__DSONDT__&&window.__DSONDT__.setMemories({json})}}catch(e){{}}"
    );
    if let Some(w) = app.get_webview_window(WEB_WEBVIEW) {
        let _ = w.eval(&js);
    }
}

/// 注入到 deepseek 页面的 JS（复原自早期可工作的「记忆前置注入」实现 c95a117）：
/// - 🧠 注入记忆：把本地记忆按相关性拼成前缀块、兼容 React 受控组件地填进输入框（用户自己按回车）；
/// - 📚 打开记忆库：藏起网页层并通知本地 UI 弹记忆库面板；
/// - 被动监听 fetch 把用户消息沉淀为 web 记忆；IPC 不可用时本地二元组打分兜底。
/// 记忆快照在初始化时烘焙进脚本（`__DSONDT_MEMORIES__` 占位符由 `inject_script` 替换）。
const INJECT_JS: &str = r##"
(function () {
  if (window.__DSONDT_INJECTED__) return;
  window.__DSONDT_INJECTED__ = true;

  var MEMORIES = __DSONDT_MEMORIES__;
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
    // 仅保留「🧠 注入记忆」按钮。「📚 打开记忆库」按钮因外部 webview 调 IPC
    // 在 Tauri 2 capability 匹配有 known issue（#10298/#10317），App 顶栏本身就有 🧠 入口，
    // 这里再放一份容易让人误以为坏的按钮，索性撤掉。
    '<button class="btn" id="inj">🧠 注入记忆 <span class="badge" id="b">0</span></button>',
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

  mount();
  setInterval(function () {
    if (document.body && !document.getElementById('__dsondt_host')) document.body.appendChild(host);
  }, 2000);
})();
"##;

/// 烘焙记忆快照：把 `__DSONDT_MEMORIES__` 占位符替换为当前记忆 JSON。
fn inject_script(memories_json: &str) -> String {
    INJECT_JS.replace("__DSONDT_MEMORIES__", memories_json)
}
