use crate::db::{Db, MemoryRow};
use crate::deepseek;
use std::sync::Mutex;
use tauri::ipc::Channel;

const KEYRING_SERVICE: &str = "com.wolfprince.dsondt";
const KEYRING_USER: &str = "deepseek-api-key";

pub struct AppState {
    pub db: Mutex<Db>,
    pub client: reqwest::Client,
}

impl AppState {
    pub fn new(db_path: &std::path::Path) -> anyhow::Result<Self> {
        let db = Db::open(db_path)?;
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(180))
            .build()?;
        Ok(Self {
            db: Mutex::new(db),
            client,
        })
    }

    pub fn get_api_key(&self) -> Result<String, String> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER).map_err(|e| e.to_string())?;
        match entry.get_password() {
            Ok(p) => Ok(p),
            Err(keyring::Error::NoEntry) => Ok(String::new()),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn set_api_key(&self, key: &str) -> Result<(), String> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER).map_err(|e| e.to_string())?;
        entry.set_password(key).map_err(|e| e.to_string())
    }

    pub async fn chat(
        &self,
        conversation_id: i64,
        content: String,
        model: String,
        use_memory: bool,
        thinking: bool,
        on_token: Channel<String>,
    ) -> Result<String, String> {
        let api_key = self.get_api_key()?;
        if api_key.is_empty() {
            return Err("未配置 API Key，请在设置中填写".into());
        }
        let client = self.client.clone();

        let history = {
            let db = self.db.lock().unwrap();
            db.get_messages(conversation_id).map_err(|e| e.to_string())?
        };

        let user_emb = if use_memory {
            match deepseek::embed(&client, &api_key, &content).await {
                Ok(v) => Some(v),
                Err(e) => {
                    eprintln!("embedding 失败（记忆将不可用）: {e}");
                    None
                }
            }
        } else {
            None
        };

        let memories = if use_memory {
            if let Some(emb) = &user_emb {
                let db = self.db.lock().unwrap();
                db.search_similar(emb, 5, 0.25).unwrap_or_default()
            } else {
                vec![]
            }
        } else {
            vec![]
        };

        let system = build_system_prompt(&memories);

        {
            let db = self.db.lock().unwrap();
            db.add_message(conversation_id, "user", &content, user_emb.as_deref())
                .map_err(|e| e.to_string())?;
            let _ = db.add_memory(&content, user_emb.as_deref(), "auto");
            let convs = db.list_conversations().map_err(|e| e.to_string())?;
            if let Some(conv) = convs.iter().find(|c| c.id == conversation_id) {
                if conv.title == "新对话" {
                    let title: String = content.chars().take(20).collect();
                    let _ = db.rename_conversation(conversation_id, &title);
                }
            }
        }

        let reply = deepseek::chat_stream(&client, &api_key, &model, &system, &history, &content, thinking, on_token)
            .await
            .map_err(|e| e.to_string())?;

        {
            let db = self.db.lock().unwrap();
            db.add_message(conversation_id, "assistant", &reply, None)
                .map_err(|e| e.to_string())?;
        }

        Ok(reply)
    }

    pub async fn add_manual_memory(&self, content: &str) -> Result<(), String> {
        let api_key = self.get_api_key()?;
        if api_key.is_empty() {
            return Err("未配置 API Key，无法生成记忆向量".into());
        }
        let client = self.client.clone();
        match deepseek::embed(&client, &api_key, content).await {
            Ok(emb) => {
                self.db
                    .lock()
                    .unwrap()
                    .add_memory(content, Some(&emb), "manual")
                    .map_err(|e| e.to_string())?;
                Ok(())
            }
            Err(e) => Err(format!("记忆向量生成失败：{e}")),
        }
    }

    pub fn list_memories(&self) -> Result<Vec<MemoryRow>, String> {
        self.db.lock().unwrap().list_memories().map_err(|e| e.to_string())
    }

    pub async fn update_memory(&self, id: i64, content: &str) -> Result<(), String> {
        let api_key = self.get_api_key()?;
        if api_key.is_empty() {
            return Err("未配置 API Key，无法重新生成记忆向量".into());
        }
        let client = self.client.clone();
        let emb = deepseek::embed(&client, &api_key, content)
            .await
            .map_err(|e| format!("记忆向量生成失败：{e}"))?;
        self.db
            .lock()
            .unwrap()
            .update_memory(id, content, Some(&emb))
            .map_err(|e| e.to_string())
    }

    pub fn delete_memory(&self, id: i64) -> Result<(), String> {
        self.db.lock().unwrap().delete_memory(id).map_err(|e| e.to_string())
    }
}

fn build_system_prompt(memories: &[String]) -> String {
    if memories.is_empty() {
        return "你是 DeepSeek，由深度求索公司创造的 AI 助手。".to_string();
    }
    let mut p = String::from(
        "你是 DeepSeek，由深度求索公司创造的 AI 助手。\n\n以下是与用户的长期记忆片段，请在回答时自然参考（不要刻意提及这些记忆的来源）：\n",
    );
    for (i, m) in memories.iter().enumerate() {
        p.push_str(&format!("{}. {}\n", i + 1, m));
    }
    p
}
