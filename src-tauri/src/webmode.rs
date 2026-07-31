//! # 单窗口双模式
//!
//! API 模式 = 主窗口里只放本地 `ui` webview（铺满，含顶栏 + 侧栏 + 聊天区）。
//! 网页模式 = 在主窗口之上叠出 `deepseek` 这个 webview，占据顶栏下方的整幅区域。
//!
//! ## 平台差异（核心坑）
//!
//! - **macOS**：Tauri 2 `unstable` 的 `Window::add_child` 正常工作，两个 webview 在同一个 NSWindow 内分层。
//!   顶栏由 `ui` webview 顶部 48px 自己画，`deepseek` webview 用 `LogicalPosition(0, 48)` 摆在它下面。
//! - **Windows / Linux**：`add_child` 第二个 webview 在 WebView2 / GTK-WebKit 下**渲染不可靠**（多数情况是根本看不到），
//!   这是 Tauri `unstable` API 在这两个平台上的已知坑。所以这里改用**独立顶级窗口**
//!   `WebviewWindowBuilder` 起一个名叫 `WEB_WEBVIEW` 的小窗口，按主窗口内容区（顶栏下方）摆位，
//!   顶栏之下整个矩形盖住主窗口对应区域，视觉上等价于"网页模式覆盖 API 模式"。

use std::sync::{Arc, Mutex};
use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, PhysicalPosition, PhysicalSize, WebviewUrl,
    Window,
};
#[cfg(target_os = "macos")]
use tauri::WebviewBuilder;
#[cfg(not(target_os = "macos"))]
use tauri::{WebviewWindow, WebviewWindowBuilder};

pub const MAIN_WINDOW: &str = "main";
pub const UI_WEBVIEW: &str = "ui";
pub const WEB_WEBVIEW: &str = "deepseek";

/// 必须与 style.css 里 `.app-topbar { height: 48px }` 保持一致。
pub const TOP_BAR_H: f64 = 48.0;

/// 当前模式状态。`active` 表示是否在网页模式；`suppressed` 表示本地要弹模态，
/// 需要临时把网页视图藏起来。
#[derive(Default)]
pub struct WebStateInner {
    pub active: bool,
    pub suppressed: bool,
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
    let inner: tauri::PhysicalSize<u32> =
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

/// 取主窗口逻辑像素位置（外屏坐标系）。仅 Win/Linux 路径使用。
#[allow(dead_code)]
pub fn window_position(window: &Window) -> (f64, f64) {
    let pos: tauri::PhysicalPosition<i32> =
        window.outer_position().unwrap_or(PhysicalPosition::new(100, 100));
    let scale = window.scale_factor().unwrap_or(1.0);
    (pos.x as f64 / scale, pos.y as f64 / scale)
}

/// 布局调整：UI webview 永远铺满主窗口；网页模式激活时再补一个 deepseek webview
/// 摆在顶栏下方。
pub fn relayout(window: &Window) {
    let (w, h) = window_size(window);
    let app = window.app_handle();

    // 本地 UI 永远铺满整个窗口（含顶栏）
    if let Some(ui) = app.get_webview(UI_WEBVIEW) {
        let _ = ui.set_position(LogicalPosition::new(0.0, 0.0));
        let _ = ui.set_size(LogicalSize::new(w, h));
    }

    if !app.state::<WebState>().is_active() {
        return;
    }

    // 网页模式下：让 deepseek 视图覆盖顶栏下方整块区域
    place_web_window(window, w, h);
}

/// 进入网页模式：若还没创建则创建 deepseek 视图，然后摆位 + 注入记忆。
pub fn activate(app: &AppHandle, memories_json: &str) -> Result<(), String> {
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

    #[cfg(target_os = "macos")]
    {
        // macOS 走 add_child：把 deepseek webview 作为主窗口的 child，定位在顶栏下方
        let url: tauri::Url = "https://chat.deepseek.com"
            .parse()
            .expect("deepseek URL 必合法");
        main.add_child(
            WebviewBuilder::new(WEB_WEBVIEW, WebviewUrl::External(url))
                .initialization_script(&inject_script(memories_json))
                .auto_resize(),
            LogicalPosition::new(0.0, TOP_BAR_H),
            LogicalSize::new(w, (h - TOP_BAR_H).max(1.0)),
        )
        .map_err(|e| format!("创建 deepseek 子视图失败：{e}"))?;
    }

    #[cfg(not(target_os = "macos"))]
    {
        // Windows / Linux 走独立顶级窗口（add_child 在 WebView2/GTK-WebKit 不可靠）
        let url: tauri::Url = "https://chat.deepseek.com"
            .parse()
            .expect("deepseek URL 必合法");
        let web_win = WebviewWindowBuilder::new(app, WEB_WEBVIEW, WebviewUrl::External(url))
            .title("DSonDT · Web")
            .position(0.0, TOP_BAR_H)
            .inner_size(w, (h - TOP_BAR_H).max(1.0))
            // 去掉原生标题栏 / 窗口装饰，避免在网页视图顶部出现一条 OS 自己的标题栏
            .decorations(false)
            // 不要从任务栏单独占一个位（它是主窗口的"内容区投影"，不是独立 App）
            .skip_taskbar(true)
            // 切到网页模式时不要偷主窗口的焦点
            .focused(false)
            // 始终叠在主窗口之上，否则会被主窗口挡住
            .always_on_top(true)
            // 用户拖主窗口边缘 resize 时 web 窗口由我们手动同步，不要让 OS 一起 resize
            .resizable(false)
            .initialization_script(&inject_script(memories_json))
            .build()
            .map_err(|e| format!("创建 deepseek 窗口失败：{e}"))?;

        // 摆到主窗口内容区对应的物理位置
        sync_web_window_position(&main, &web_win);
    }

    state.set_active(true);
    state.set_suppressed(false);
    Ok(())
}

/// 退出网页模式：deepseek 视图隐藏但保留，页面与登录态都还在，下次切回秒级恢复。
pub fn deactivate(app: &AppHandle) {
    let state = app.state::<WebState>();
    state.set_active(false);
    state.set_suppressed(false);

    #[cfg(target_os = "macos")]
    {
        if let Some(w) = app.get_webview(WEB_WEBVIEW) {
            // macOS child webview 没独立 close 概念；直接关掉下次重新 add_child
            let _ = w.close();
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        if let Some(w) = app.get_webview_window(WEB_WEBVIEW) {
            let _ = w.hide();
        }
    }
}

pub fn is_active(app: &AppHandle) -> bool {
    app.state::<WebState>().is_active()
}

/// 临时隐藏 / 恢复 deepseek 视图（用于本地弹模态）。
pub fn set_suppressed(app: &AppHandle, suppressed: bool) {
    let state = app.state::<WebState>();
    state.set_suppressed(suppressed);
    if !state.is_active() {
        return;
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(w) = app.get_webview(WEB_WEBVIEW) {
            if suppressed {
                let _ = w.hide();
            } else {
                let _ = w.show();
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        if let Some(w) = app.get_webview_window(WEB_WEBVIEW) {
            if suppressed {
                let _ = w.hide();
            } else {
                let _ = w.show();
            }
        }
    }
}

/// 把最新记忆 JSON 推给当前正在显示的 deepseek 视图。
pub fn push_memories(app: &AppHandle, json: &str) {
    let escaped = json.replace('\\', "\\\\").replace('\'', "\\'");
    let js = format!(
        "window.__DSONDT_PUSH_MEMORIES__ && window.__DSONDT_PUSH_MEMORIES__('{escaped}')"
    );
    #[cfg(target_os = "macos")]
    {
        if let Some(w) = app.get_webview(WEB_WEBVIEW) {
            let _ = w.eval(&js);
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        if let Some(w) = app.get_webview_window(WEB_WEBVIEW) {
            let _ = w.eval(&js);
        }
    }
}

/// 主窗口移动 / 缩放后，网页模式激活时把 web 窗口同步到主窗口内容区。
fn place_web_window(_window: &Window, w: f64, h: f64) {
    #[cfg(not(target_os = "macos"))]
    {
        let app = _window.app_handle();
        if let Some(web) = app.get_webview_window(WEB_WEBVIEW) {
            // 先按逻辑像素 resize，再把 web 窗口贴到主窗口内容区
            let _ = web.set_size(LogicalSize::new(w, (h - TOP_BAR_H).max(1.0)));
            sync_web_window_position(_window, &web);
        }
    }
    #[cfg(target_os = "macos")]
    {
        let _ = (w, h);
    }
}

#[cfg(not(target_os = "macos"))]
fn sync_web_window_position(main: &Window, web: &WebviewWindow) {
    let (mx, my) = window_position(main);
    let scale = main.scale_factor().unwrap_or(1.0);
    let _ = web.set_position(PhysicalPosition::new(
        (mx * scale) as i32,
        ((my + TOP_BAR_H) * scale) as i32,
    ));
}

/// 注入到 deepseek 页面的 JS：监听 fetch、构建本地记忆快照、把用户消息被动沉淀为 web 记忆。
fn inject_script(memories_json: &str) -> String {
    let memories_json_json = memories_json; // 命名占位，避免和 format 占位符冲突
    format!(
        r#"
(function () {{
  if (window.__DSONDT_INSTALLED__) return;
  window.__DSONDT_INSTALLED__ = true;
  const INITIAL_MEMORIES = {memories_json_json};
  let currentMemories = INITIAL_MEMORIES;

  function renderSystemBlock() {{
    if (!currentMemories || currentMemories.length === 0) return null;
    const lines = currentMemories.map(function (m) {{ return '- ' + m.content; }});
    return [
      '[DSonDT 长期记忆 — 来自你本地]',
      '下列内容是用户之前跟你说过的关键背景，AI 应当主动参考（不要复述给用户）：',
    ].concat(lines).join('\n');
  }}

  // Rust 端通过 eval 调用本函数推新记忆进来
  window.__DSONDT_PUSH_MEMORIES__ = function (json) {{
    try {{
      currentMemories = JSON.parse(json);
    }} catch (e) {{ console.error('[DSonDT] push memories parse failed', e); }}
  }};

  const origFetch = window.fetch;
  window.fetch = async function (input, init) {{
    try {{
      const url = typeof input === 'string' ? input : (input && input.url) || '';
      if (url.includes('/api/v0/chat/completion') || url.includes('chat/completion')) {{
        let body = init && init.body;
        if (typeof body === 'string') {{
          try {{
            const data = JSON.parse(body);
            if (Array.isArray(data && data.messages)) {{
              const last = data.messages[data.messages.length - 1];
              if (last && last.role === 'user' && last.content) {{
                try {{
                  await window.__TAURI_INTERNALS__.invoke('add_web_memory', {{ content: last.content }});
                }} catch (_) {{}}
              }}
            }}
          }} catch (_) {{}}
        }}
      }}
    }} catch (_) {{}}
    return origFetch.apply(this, arguments);
  }};
}})();
"#,
    )
}
