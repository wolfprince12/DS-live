#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod db;
mod deepseek;
mod memory;
mod state;
mod webmode;

use state::AppState;
#[cfg(target_os = "macos")]
use tauri::TitleBarStyle;
use tauri::{
    WebviewBuilder, WebviewUrl,
    window::WindowBuilder,
    LogicalPosition, LogicalSize, Manager, WindowEvent,
};
use webmode::{MAIN_WINDOW, UI_WEBVIEW, WebState};

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
    let spawned = std::process::Command::new("cmd")
        .args(["/C", "start", "", &url])
        .spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
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
fn open_web_mode(app: tauri::AppHandle, state: tauri::State<AppState>) -> Result<(), String> {
    let json = memories_json(&state);
    webmode::activate(&app, &json)
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
    if let Some(ui) = app.get_webview(UI_WEBVIEW) {
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

            // 先造一个没有 webview 的纯容器窗口，再把本地 UI 作为子视图塞进去。
            // 网页模式下再追加一个官方 deepseek 子视图，二者共处一窗。
            //
            // macOS：把原生标题栏藏掉（Overlay + hiddenTitle），让红黄绿按钮浮在我们自定义顶栏之上；
            //        否则会出现「双品牌」+ 「双标题栏」+ 顶栏被原生栏挤压变形的视觉问题。
            // Win/Linux：标题栏正常使用，本地 UI 还是从 y=0 开始铺。
            #[allow(unused_mut)] // mut 仅 macOS 分支用到
            let mut win_builder = WindowBuilder::new(&*app, MAIN_WINDOW)
                .title("DSonDT")
                // 初始窗口尺寸：按用户期望的「正常可用」尺寸开，不要每次都得手动拉大。
                .inner_size(1280.0, 820.0)
                .min_inner_size(960.0, 640.0)
                .resizable(true);
            #[cfg(target_os = "macos")]
            {
                win_builder = win_builder
                    .title_bar_style(TitleBarStyle::Overlay)
                    .hidden_title(true);
            }
            let window = win_builder.build().map_err(|e| e.to_string())?;

            let (w, h) = webmode::window_size(&window);
            window
                .add_child(
                    WebviewBuilder::new(UI_WEBVIEW, WebviewUrl::App("index.html".into())),
                    LogicalPosition::new(0.0, 0.0),
                    LogicalSize::new(w, h),
                )
                .map_err(|e| format!("创建本地 UI 视图失败：{e}"))?;

            webmode::relayout(&window);

            // 窗口尺寸 / 位置 / 缩放变化时，重新摆位两个子视图，否则它们不会跟着动。
            // Win/Linux 还需要同步：网页模式下的 web 顶级窗口要跟主窗口位置/尺寸变化。
            window.on_window_event(move |event| {
                match event {
                    WindowEvent::Resized(_) | WindowEvent::Moved(_) => {
                        if let Some(win) = handle.get_window(MAIN_WINDOW) {
                            webmode::relayout(&win);
                        }
                    }
                    WindowEvent::CloseRequested { .. } => {
                        // Win/Linux 模式下网页视图是独立顶级窗口，主窗口关闭时
                        // 要主动把它也关掉，否则 app 不会退出（还有一个窗口活着）。
                        if let Some(web) = handle.get_window(webmode::WEB_WEBVIEW) {
                            let _ = web.close();
                        }
                    }
                    _ => {}
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
            set_webview_suppressed,
            open_memory_panel,
            sync_web_memories,
            search_memories,
            add_web_memory
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
