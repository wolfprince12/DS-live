use crate::db::{Db, MemoryRow};
use crate::deepseek;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::ipc::Channel;

const KEYRING_SERVICE: &str = "com.wolfprince.dsondt";
const KEYRING_USER: &str = "deepseek-api-key";

/// 本地回退文件的混淆掩码。**这不是加密**，只是避免 Key 以肉眼可读的明文躺在磁盘上。
/// 真正的机密性依赖钥匙串；文件回退是为了在钥匙串不可用时不至于「存了读不回」。
const MASK: &[u8] = b"DSonDT/v1/local-apikey-mask";

#[derive(serde::Serialize)]
pub struct ApiKeyStatus {
    pub saved: bool,
    /// 打码后的 Key，仅用于让用户确认「确实存住了」，如 `sk-abc****wxyz`
    pub masked: String,
    /// 是否落在钥匙串里（false 表示只有本地回退文件）
    pub in_keyring: bool,
}

pub struct AppState {
    pub db: Mutex<Db>,
    pub client: reqwest::Client,
    key_file: PathBuf,
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
            key_file: db_path.with_file_name("apikey.bin"),
        })
    }

    fn mask_bytes(data: &[u8]) -> Vec<u8> {
        data.iter()
            .enumerate()
            .map(|(i, b)| b ^ MASK[i % MASK.len()])
            .collect()
    }

    fn read_key_file(&self) -> String {
        match std::fs::read(&self.key_file) {
            Ok(raw) => String::from_utf8(Self::mask_bytes(&raw)).unwrap_or_default(),
            Err(_) => String::new(),
        }
    }

    fn write_key_file(&self, key: &str) -> Result<(), String> {
        if key.is_empty() {
            let _ = std::fs::remove_file(&self.key_file);
            return Ok(());
        }
        std::fs::write(&self.key_file, Self::mask_bytes(key.as_bytes()))
            .map_err(|e| format!("写入本地 Key 文件失败：{e}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&self.key_file, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }

    fn keyring_key(&self) -> Option<String> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER).ok()?;
        match entry.get_password() {
            Ok(p) if !p.trim().is_empty() => Some(p),
            _ => None,
        }
    }

    /// 读取 Key。**永远不返回 Err**——读不到就是空字符串。
    ///
    /// 为什么要有文件回退：本 App 用 ad-hoc 签名分发（无 Apple 开发者账号），
    /// 每次重新构建二进制的 cdhash 都会变，macOS 钥匙串条目的 ACL 认的是旧签名，
    /// 新版本读取时会被系统直接拒绝。表现就是「填了 Key，重启后又说没配置」。
    pub fn get_api_key(&self) -> Result<String, String> {
        if let Some(k) = self.keyring_key() {
            return Ok(k);
        }
        Ok(self.read_key_file())
    }

    /// 写入 Key。本地文件是主力（一定成功），钥匙串是尽力而为。
    pub fn set_api_key(&self, key: &str) -> Result<(), String> {
        let key = key.trim();
        self.write_key_file(key)?;
        if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER) {
            if key.is_empty() {
                let _ = entry.delete_credential();
            } else {
                let _ = entry.set_password(key);
            }
        }
        Ok(())
    }

    pub fn api_key_status(&self) -> ApiKeyStatus {
        let in_keyring = self.keyring_key().is_some();
        let key = self.get_api_key().unwrap_or_default();
        if key.is_empty() {
            return ApiKeyStatus {
                saved: false,
                masked: String::new(),
                in_keyring: false,
            };
        }
        let chars: Vec<char> = key.chars().collect();
        let masked = if chars.len() <= 12 {
            format!("{}••••", chars.iter().take(3).collect::<String>())
        } else {
            format!(
                "{}••••{}",
                chars[..6].iter().collect::<String>(),
                chars[chars.len() - 4..].iter().collect::<String>()
            )
        };
        ApiKeyStatus {
            saved: true,
            masked,
            in_keyring,
        }
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
            let db = self.db.lock().unwrap();
            let hits = match &user_emb {
                Some(emb) => db.search_similar(emb, 5, 0.25).unwrap_or_default(),
                None => vec![],
            };
            if hits.is_empty() {
                // 向量不可用或没命中，退关键词检索
                db.search_keyword(&content, 5).unwrap_or_default()
            } else {
                hits
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

    /// 尽力生成 embedding。没有 Key 或调用失败都不算错误——
    /// 记忆照样存，只是检索时退化为关键词匹配（网页模式的零成本路径依赖这一点）。
    async fn try_embed(&self, content: &str) -> Option<Vec<f32>> {
        let api_key = self.get_api_key().ok()?;
        if api_key.is_empty() {
            return None;
        }
        let client = self.client.clone();
        match deepseek::embed(&client, &api_key, content).await {
            Ok(v) => Some(v),
            Err(e) => {
                eprintln!("embedding 失败，记忆降级为关键词检索: {e}");
                None
            }
        }
    }

    pub async fn add_manual_memory(&self, content: &str) -> Result<(), String> {
        let emb = self.try_embed(content).await;
        self.db
            .lock()
            .unwrap()
            .add_memory(content, emb.as_deref(), "manual")
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// 网页模式沉淀下来的记忆，来源标记为 web，去重后写入。
    pub async fn add_web_memory(&self, content: &str) -> Result<(), String> {
        let content = content.trim();
        if content.len() < 4 {
            return Ok(());
        }
        {
            let db = self.db.lock().unwrap();
            if let Ok(existing) = db.list_memories() {
                if existing.iter().any(|m| m.content == content) {
                    return Ok(());
                }
            }
        }
        let emb = self.try_embed(content).await;
        self.db
            .lock()
            .unwrap()
            .add_memory(content, emb.as_deref(), "web")
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn list_memories(&self) -> Result<Vec<MemoryRow>, String> {
        self.db.lock().unwrap().list_memories().map_err(|e| e.to_string())
    }

    /// 记忆检索：有 Key 走向量，无 Key/失败自动退关键词。两条路都不会空手而归。
    pub async fn search_memories(&self, query: &str, top_k: usize) -> Result<Vec<String>, String> {
        if let Some(emb) = self.try_embed(query).await {
            let db = self.db.lock().unwrap();
            let hits = db.search_similar(&emb, top_k, 0.25).unwrap_or_default();
            if !hits.is_empty() {
                return Ok(hits);
            }
        }
        self.db
            .lock()
            .unwrap()
            .search_keyword(query, top_k)
            .map_err(|e| e.to_string())
    }

    pub async fn update_memory(&self, id: i64, content: &str) -> Result<(), String> {
        let emb = self.try_embed(content).await;
        self.db
            .lock()
            .unwrap()
            .update_memory(id, content, emb.as_deref())
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
