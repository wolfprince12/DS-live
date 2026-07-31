#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod db;
mod deepseek;
mod memory;
mod state;

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
            delete_memory
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
