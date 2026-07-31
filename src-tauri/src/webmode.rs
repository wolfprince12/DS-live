//! 网页模式：内嵌官方 chat.deepseek.com，用注入脚本把本地记忆库前置到输入框。
//!
//! 设计要点：
//! - 只做「前置注入」，**不代替用户按发送键**。性质等同输入法/剪贴板辅助，不越界。
//! - 注入脚本被动 hook fetch 读取页面自己发出的请求，不伪造请求、不窃取 token。
//! - 记忆快照在建窗时烘焙进脚本；主窗口增删改记忆后由 Rust 主动 eval 推送更新。
//! - IPC 不可用时自动降级为脚本内本地打分，功能不中断。

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

pub const WEB_WINDOW_LABEL: &str = "deepseek";
const DEEPSEEK_URL: &str = "https://chat.deepseek.com/";

/// 伪装成真实浏览器 UA，避免 WebView 默认 UA 触发风控挑战。
#[cfg(target_os = "macos")]
const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Safari/605.1.15";
#[cfg(target_os = "windows")]
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";
#[cfg(all(unix, not(target_os = "macos")))]
const UA: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

/// 打开（或聚焦）网页模式窗口。`memories_json` 是 `[{content, origin}]` 的 JSON 数组。
pub fn open(app: &AppHandle, memories_json: &str) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(WEB_WINDOW_LABEL) {
        let _ = win.show();
        let _ = win.set_focus();
        push_memories(app, memories_json);
        return Ok(());
    }
    let script = INJECT_JS.replace("__DSONDT_MEMORIES__", memories_json);
    let url = DEEPSEEK_URL
        .parse()
        .map_err(|e| format!("URL 解析失败：{e}"))?;
    WebviewWindowBuilder::new(app, WEB_WINDOW_LABEL, WebviewUrl::External(url))
        .title("DeepSeek · 网页模式（DSonDT 记忆增强）")
        .inner_size(1180.0, 840.0)
        .min_inner_size(900.0, 600.0)
        .user_agent(UA)
        .initialization_script(&script)
        .build()
        .map_err(|e| format!("创建网页模式窗口失败：{e}"))?;
    Ok(())
}

/// 主窗口改动记忆后，把最新快照推给网页窗口（若已打开）。
pub fn push_memories(app: &AppHandle, memories_json: &str) {
    if let Some(win) = app.get_webview_window(WEB_WINDOW_LABEL) {
        let js = format!(
            "try{{window.__DSONDT__&&window.__DSONDT__.setMemories({memories_json})}}catch(e){{}}"
        );
        let _ = win.eval(&js);
    }
}

pub fn is_open(app: &AppHandle) -> bool {
    app.get_webview_window(WEB_WINDOW_LABEL).is_some()
}

const INJECT_JS: &str = r##"
(function () {
  if (window.__DSONDT_INJECTED__) return;
  window.__DSONDT_INJECTED__ = true;

  var MEMORIES = __DSONDT_MEMORIES__;
  var TOP_K = 5;
  var autoSink = true;

  /* ---------- IPC（remote 页面可能不可用，全部走降级） ---------- */
  function invoke(cmd, args) {
    try {
      var it = window.__TAURI_INTERNALS__;
      if (it && typeof it.invoke === 'function') return it.invoke(cmd, args || {});
    } catch (e) {}
    return Promise.reject(new Error('ipc-unavailable'));
  }

  /* ---------- 本地打分兜底：字符二元组重合度 ---------- */
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

  /* ---------- 定位输入框 ---------- */
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

  /* React 受控组件必须走原生 setter + input 事件，直接赋值会被回滚 */
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

  /* ---------- 悬浮控件（Shadow DOM 隔离，不污染官网样式） ---------- */
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
    '<button class="btn sec" id="lib">📚 打开记忆库</button>',
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
  shadow.getElementById('lib').addEventListener('click', function () {
    invoke('focus_main_window', {}).catch(function () {
      toast('请切回 DSonDT 主窗口打开记忆库');
    });
  });

  document.addEventListener('keydown', function (e) {
    if ((e.metaKey || e.ctrlKey) && (e.key === 'm' || e.key === 'M')) {
      e.preventDefault();
      doInject();
    }
  });

  /* ---------- 被动沉淀：旁听页面自己发出的请求，不伪造 ---------- */
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
  /* 官网是 SPA，路由切换可能重建 body，定期确保控件还在 */
  setInterval(function () {
    if (document.body && !document.getElementById('__dsondt_host')) document.body.appendChild(host);
  }, 2000);
})();
"##;
