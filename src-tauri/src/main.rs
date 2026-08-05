#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod db;
mod deepseek;
mod memory;
mod state;
mod update;
mod webmode;

use state::AppState;
#[cfg(target_os = "macos")]
use tauri::TitleBarStyle;
use tauri::{WebviewWindowBuilder, WebviewUrl, Manager, WindowEvent};
use webmode::{MAIN_WINDOW, WebState};

#[tauri::command]
fn has_api_key(state: tauri::State<AppState>) -> Result<bool, String> {
    state.get_api_key().map(|s| !s.is_empty())
}

#[tauri::command]
fn api_key_status(state: tauri::State<AppState>) -> state::ApiKeyStatus {
    state.api_key_status()
}

#[tauri::command]
fn set_api_key(state: tauri::State<AppState>, key: String) -> Result<(), String> {
    state.set_api_key(&key)
}

#[tauri::command]
fn get_conversations(state: tauri::State<AppState>) -> Result<Vec<db::Conversation>, String> {
    state.db.lock().unwrap().list_conversations().map_err(|e| e.to_string())
}

#[tauri::command]
fn create_conversation(state: tauri::State<AppState>, title: Option<String>) -> Result<db::Conversation, String> {
    state.db.lock().unwrap().create_conversation(title).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_conversation(state: tauri::State<AppState>, id: i64) -> Result<(), String> {
    state.db.lock().unwrap().delete_conversation(id).map_err(|e| e.to_string())
}

#[tauri::command]
fn rename_conversation(state: tauri::State<AppState>, id: i64, title: String) -> Result<(), String> {
    state.db.lock().unwrap().rename_conversation(id, &title).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_messages(state: tauri::State<AppState>, conversation_id: i64) -> Result<Vec<db::Message>, String> {
    state.db.lock().unwrap().get_messages(conversation_id).map_err(|e| e.to_string())
}

#[tauri::command]
async fn chat(
    state: tauri::State<'_, AppState>,
    conversation_id: i64,
    content: String,
    model: String,
    use_memory: bool,
    thinking: bool,
    on_token: tauri::ipc::Channel<String>,
) -> Result<String, String> {
    state.chat(conversation_id, content, model, use_memory, thinking, on_token).await
}

#[tauri::command]
fn export_conversation(state: tauri::State<AppState>, conversation_id: i64) -> Result<String, String> {
    state.db.lock().unwrap().export_conversation(conversation_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn import_conversation(state: tauri::State<AppState>, json: String) -> Result<db::Conversation, String> {
    state.db.lock().unwrap().import_conversation(&json).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_memories(state: tauri::State<AppState>) -> Result<Vec<db::MemoryRow>, String> {
    state.list_memories()
}

#[tauri::command]
async fn add_manual_memory(state: tauri::State<'_, AppState>, content: String) -> Result<(), String> {
    state.add_manual_memory(&content).await
}

#[tauri::command]
async fn update_memory(state: tauri::State<'_, AppState>, id: i64, content: String) -> Result<(), String> {
    state.update_memory(id, &content).await
}

#[tauri::command]
fn delete_memory(state: tauri::State<AppState>, id: i64) -> Result<(), String> {
    state.delete_memory(id)
}

/// 用系统默认浏览器打开外部链接。
/// Tauri WebView 内 `window.open` 会被拦截，必须由 Rust 侧调用系统命令。
#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("仅允许打开 http/https 链接".into());
    }
    #[cfg(target_os = "macos")]
    let spawned = std::process::Command::new("open").arg(&url).spawn();
    #[cfg(target_os = "windows")]
    let spawned = std::process::Command::new("cmd").args(["/c", "start", "", &url]).spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let spawned = std::process::Command::new("xdg-open").arg(&url).spawn();

    spawned.map(|_| ()).map_err(|e| format!("打开浏览器失败：{e}"))
}

/// 把当前记忆库序列化成注入脚本能直接吃的 JSON。
fn memories_json(state: &AppState) -> String {
    match state.list_memories() {
        Ok(list) => {
            let brief: Vec<serde_json::Value> = list
                .iter()
                .map(|m| serde_json::json!({ "content": m.content, "origin": m.origin }))
                .collect();
            serde_json::to_string(&brief).unwrap_or_else(|_| "[]".into())
        }
        Err(_) => "[]".into(),
    }
}

/// 打开网页模式：在**同一个窗口**内激活右侧的官方 chat.deepseek.com 子视图，并注入本地记忆增强层。
#[tauri::command]
fn open_web_mode(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    web_state: tauri::State<WebState>,
) -> Result<(), String> {
    let json = memories_json(&state);
    let platform = if cfg!(target_os = "windows") { "win" } else { "mac" };
    webmode::activate(&app, &json, &web_state.home_url(), platform)
}

#[tauri::command]
fn web_mode_open(app: tauri::AppHandle) -> bool {
    webmode::is_active(&app)
}

/// 切回 API 模式：隐藏远程子视图（页面与登录态都保留，随时可切回）。
#[tauri::command]
fn deactivate_web_mode(app: tauri::AppHandle) {
    webmode::deactivate(&app);
}

/// 读取并清空 pending_api 标志（Windows 网页模式「💬 返回」后纠正 store.mode 用）。
#[tauri::command]
fn take_pending_api(app: tauri::AppHandle) -> bool {
    webmode::take_pending_api(&app)
}

/// 本地要弹模态框了，先把压在上面的远程 webview 藏起来，关闭后再恢复。
#[tauri::command]
fn set_webview_suppressed(app: tauri::AppHandle, suppressed: bool) {
    webmode::set_suppressed(&app, suppressed);
}

/// 供网页模式里注入的「📚 编辑记忆库」按钮调用：
/// 藏起远程视图并通知本地 UI 打开记忆库弹窗（弹窗关闭后再由前端恢复显示）。
#[tauri::command]
fn open_memory_panel(app: tauri::AppHandle) {
    webmode::set_suppressed(&app, true);
    if let Some(ui) = app.get_webview(MAIN_WINDOW) {
        let _ = ui.eval("window.dispatchEvent(new CustomEvent('dsondt:open-memory'))");
    }
}

/// 主窗口改动记忆后调用，把最新快照推给已打开的网页视图。
#[tauri::command]
fn sync_web_memories(app: tauri::AppHandle, state: tauri::State<AppState>) {
    let json = memories_json(&state);
    webmode::push_memories(&app, &json);
}

/// 供注入脚本调用：检索记忆（无 API Key 时自动走关键词路径）。
#[tauri::command]
async fn search_memories(
    state: tauri::State<'_, AppState>,
    query: String,
    top_k: Option<usize>,
) -> Result<Vec<String>, String> {
    state.search_memories(&query, top_k.unwrap_or(5)).await
}

/// 供注入脚本调用：把网页模式里发出的消息被动沉淀为记忆。
#[tauri::command]
async fn add_web_memory(state: tauri::State<'_, AppState>, content: String) -> Result<(), String> {
    state.add_web_memory(&content).await
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let dir = app
                .path()
                .app_data_dir()
                .expect("无法获取应用数据目录");
            std::fs::create_dir_all(&dir).ok();
            let db_path = dir.join("dsondt.db");
            let app_state = AppState::new(&db_path).map_err(|e| e.to_string())?;
            app.manage(app_state);
            // 单窗口双视图的状态：当前是否在网页模式、是否临时藏起远程视图
            app.manage(WebState::default());

            let handle = app.handle().clone();

            // 直接把本地 UI 作为窗口的「主 webview」创建（加载 index.html）。
            // 关键：Tauri 2 的 -webkit-app-region: drag 只在「主 webview」上生效，
            // 子 webview（add_child 出来的）上的拖动 CSS 会被系统无视——这正是之前
            // 鼠标拖不动窗口的根因。所以 ui 必须是主 webview，deepseek 才是叠加的子视图。
            // 网页模式下再追加一个官方 deepseek 子视图，二者共处一窗。
            //
            // 自定义 48px 顶栏（logo + 模式标签 + 🧠⚙ℹ），两个平台的 App 内部 UI 完全一致：
            //   - macOS：TitleBarStyle::Overlay + hidden_title(true) 把原生红黄绿浮在顶栏左侧
            //            → style.css 给顶栏 padding-left:80px 让位；
            //   - Windows：decorations(true) 保留系统原生标题栏（拖拽 / 最小化 / 关闭由 OS 负责，
            //            彻底规避 WebView2 上自绘按钮点击失灵的问题）；其余 UI 与 macOS 完全一致。
            // 两端 App 自身 UI（顶栏内容/侧栏/聊天/按钮）100% 一致；窗口控制按钮：
            // mac 为 OS 原生浮左、Windows 为前端自绘同款浮左，视觉一致。

            // Windows 网页模式用「主 webview 整页导航」实现单窗口切换，返回 API 模式时
            // 要把主 webview 导回本地首页，因此必须拿到**真正的**本地首页 URL。
            // 这里在 builder 上挂 on_navigation，在首页真正发起导航时捕获它。
            //
            // 不能在 build() 之后立刻用 `vw.url()` 去取：那一刻页面尚未开始加载，
            // 拿到的是 about:blank / 空串，会让「💬 返回 API」把主 webview 导到白板，
            // 且本地 UI 丢失后再也切不回网页模式（本次修复的根因）。
            //
            // WebState 内部是 Arc<Mutex<..>>，clone 出的句柄与 app.state::<WebState>()
            // 共享同一份状态，闭包里 set_home_url 会被 webmode::deactivate 读到。
            let ws_for_nav = app.state::<WebState>().inner().clone();

            #[allow(unused_mut)] // mut 仅 macOS/Windows 分支用到
            let mut win_builder = WebviewWindowBuilder::new(
                &*app,
                MAIN_WINDOW,
                WebviewUrl::App("index.html".into()),
            )
            .title("DSonDT")
            // 初始窗口尺寸：按用户期望的「正常可用」尺寸开，不要每次都得手动拉大。
            .inner_size(1280.0, 820.0)
            .min_inner_size(960.0, 640.0)
            .resizable(true)
            .on_navigation(move |url| {
                // 只记录本地首页：Windows 打包态是 http://tauri.localhost，
                // 资源协议是 asset.localhost，dev 下是 http://localhost:端口。
                // chat.deepseek.com 等外部页不匹配，因此不会覆盖 home_url。
                let host = url.host_str().unwrap_or("");
                if host.contains("localhost") || host.contains("asset") {
                    // 必须去掉 fragment 再记录：「💬 返回」是导航到 <home>#return-api，
                    // 这一跳同样会触发本回调。若原样记下，第二次 🌐→💬 就会拼成
                    // <home>#return-api#return-api，而 ui.ts 用的是严格相等
                    // `location.hash === '#return-api'`，判等失败 → 模式纠正失效，
                    // localStorage 里残留的 'web' 会让首屏又弹回 DeepSeek。
                    //
                    // ⚠️ 承重代码，勿轻改：Windows 返回路径上 pending_api 恒为 false
                    // （该路径是纯前端导航，deactivate() 根本不会被调用，也就没人去
                    // set_pending_api(true)），因此 take_pending_api() 那条兜底在 Windows 上
                    // 形同虚设，模式纠正 100% 依赖 ui.ts 的 hash 分支这一条路，无冗余。
                    // 改动 hash 约定时（webmode.rs 的 '#return-api' 与 ui.ts 的判等，
                    // 以及这里的去 fragment）三处必须同步，否则「💬 返回」会静默失效。
                    let mut clean = url.clone();
                    clean.set_fragment(None);
                    ws_for_nav.set_home_url(clean.to_string());

                    // Windows 的「💬 返回」是注入脚本里的纯前端整页导航
                    // （webmode.rs: window.location.href = HOME_URL + '#return-api'），
                    // 不会经过 deactivate_web_mode 命令，而 active 只在 deactivate() 里被置 false，
                    // 于是它会永久残留为 true → 下次点 🌐 时 activate() 被开头的
                    // `if state.is_active() { return Ok(()) }` 提前吃掉，页面纹丝不动。
                    // 主 webview 回到本地页 == 已退出网页模式，这里同步复位（也让
                    // web_mode_open() 的返回值恢复准确）。
                    // 仅限 Windows：macOS/Linux 的 deepseek 是叠加子 webview，
                    // 主 webview 在网页模式下本来就停在本地页，复位会误清 active。
                    #[cfg(target_os = "windows")]
                    ws_for_nav.set_active(false);
                }
                // 本回调只做「记录」，一律放行导航（含切网页模式时的 deepseek）。
                true
            });
            #[cfg(target_os = "macos")]
            {
                win_builder = win_builder
                    .title_bar_style(TitleBarStyle::Overlay)
                    .hidden_title(true);
            }
            #[cfg(target_os = "windows")]
            {
                // Windows 用原生标题栏（decorations=true）：拖拽 / 最小化 / 关闭全部交给 OS，
                // 不再依赖失效的 -webkit-app-region 与前端自绘红黄绿（旧方案在 WebView2 上
                // 会让整窗点击失灵，正是上一版「按钮全死、窗口拖不动」的根因）。
                // 网页模式走主 webview 整页导航，🧠/💬 浮层与记忆注入由 activate 线程反复 eval 挂载。
                win_builder = win_builder.decorations(true);
            }
            let ww = win_builder.build().map_err(|e| e.to_string())?;

            // "main" 同时是窗口与主 webview 的标签；通过窗口句柄做初始布局。
            let main_win = app
                .get_window(MAIN_WINDOW)
                .ok_or_else(|| "主窗口创建后未注册".to_string())?;
            webmode::relayout(&main_win);

            // 窗口缩放时，重新摆位本地 UI 与（网页模式下）deepseek 子视图，否则它们不会跟着变大变小。
            ww.on_window_event(move |event| {
                if let WindowEvent::Resized(_) = event {
                    if let Some(win) = handle.get_window(MAIN_WINDOW) {
                        webmode::relayout(&win);
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            has_api_key,
            set_api_key,
            api_key_status,
            get_conversations,
            create_conversation,
            delete_conversation,
            rename_conversation,
            get_messages,
            chat,
            export_conversation,
            import_conversation,
            list_memories,
            add_manual_memory,
            update_memory,
            delete_memory,
            open_url,
            open_web_mode,
            web_mode_open,
            deactivate_web_mode,
            take_pending_api,
            set_webview_suppressed,
            open_memory_panel,
            sync_web_memories,
            search_memories,
            add_web_memory,
            update::check_update,
            update::get_version
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
