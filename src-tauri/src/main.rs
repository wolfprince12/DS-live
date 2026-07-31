#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod db;
mod deepseek;
mod memory;
mod state;
mod webmode;

use state::AppState;
use tauri::Manager;

#[tauri::command]
fn has_api_key(state: tauri::State<AppState>) -> Result<bool, String> {
    state.get_api_key().map(|s| !s.is_empty())
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

/// 打开网页模式：内嵌官方 chat.deepseek.com，并注入本地记忆增强层。
#[tauri::command]
fn open_web_mode(app: tauri::AppHandle, state: tauri::State<AppState>) -> Result<(), String> {
    let json = memories_json(&state);
    webmode::open(&app, &json)
}

#[tauri::command]
fn web_mode_open(app: tauri::AppHandle) -> bool {
    webmode::is_open(&app)
}

/// 主窗口改动记忆后调用，把最新快照推给网页窗口。
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

#[tauri::command]
fn focus_main_window(app: tauri::AppHandle) -> Result<(), String> {
    let win = app.get_webview_window("main").ok_or("找不到主窗口")?;
    win.show().map_err(|e| e.to_string())?;
    win.set_focus().map_err(|e| e.to_string())
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
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            has_api_key,
            set_api_key,
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
            sync_web_memories,
            search_memories,
            add_web_memory,
            focus_main_window
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
