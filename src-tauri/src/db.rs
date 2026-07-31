use rusqlite::{params, Connection};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct Conversation {
    pub id: i64,
    pub title: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Message {
    pub id: i64,
    pub conversation_id: i64,
    pub role: String,
    pub content: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemoryRow {
    pub id: i64,
    pub content: String,
    pub origin: String,
    pub created_at: i64,
    pub updated_at: i64,
}

pub struct Db {
    conn: Connection,
}

/// 切字符二元组，忽略空白与大小写。中英文都适用。
fn bigrams(s: &str) -> Vec<String> {
    let chars: Vec<char> = s
        .to_lowercase()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    if chars.len() < 2 {
        return chars.iter().map(|c| c.to_string()).collect();
    }
    chars.windows(2).map(|w| w.iter().collect()).collect()
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl Db {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS conversations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL DEFAULT '新对话',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                conversation_id INTEGER NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                embedding TEXT,
                created_at INTEGER NOT NULL,
                FOREIGN KEY(conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
            );
            CREATE TABLE IF NOT EXISTS memories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                content TEXT NOT NULL,
                embedding TEXT,
                origin TEXT NOT NULL DEFAULT 'auto',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_messages_conv ON messages(conversation_id);
            CREATE INDEX IF NOT EXISTS idx_memories_emb ON memories(embedding);",
        )?;
        Ok(Self { conn })
    }

    pub fn list_conversations(&self) -> rusqlite::Result<Vec<Conversation>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, title, created_at, updated_at FROM conversations ORDER BY updated_at DESC")?;
        let rows = stmt.query_map([], |r| {
            Ok(Conversation {
                id: r.get(0)?,
                title: r.get(1)?,
                created_at: r.get(2)?,
                updated_at: r.get(3)?,
            })
        })?;
        rows.collect()
    }

    pub fn create_conversation(&self, title: Option<String>) -> rusqlite::Result<Conversation> {
        let t = now();
        let title = title.unwrap_or_else(|| "新对话".to_string());
        self.conn.execute(
            "INSERT INTO conversations (title, created_at, updated_at) VALUES (?1, ?2, ?3)",
            params![title, t, t],
        )?;
        let id = self.conn.last_insert_rowid();
        Ok(Conversation {
            id,
            title,
            created_at: t,
            updated_at: t,
        })
    }

    pub fn delete_conversation(&self, id: i64) -> rusqlite::Result<()> {
        self.conn.execute("DELETE FROM conversations WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn rename_conversation(&self, id: i64, title: &str) -> rusqlite::Result<()> {
        let t = now();
        self.conn.execute(
            "UPDATE conversations SET title = ?1, updated_at = ?2 WHERE id = ?3",
            params![title, t, id],
        )?;
        Ok(())
    }

    fn touch_conversation(&self, id: i64) -> rusqlite::Result<()> {
        let t = now();
        self.conn
            .execute("UPDATE conversations SET updated_at = ?1 WHERE id = ?2", params![t, id])?;
        Ok(())
    }

    pub fn get_messages(&self, conversation_id: i64) -> rusqlite::Result<Vec<Message>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, conversation_id, role, content, created_at FROM messages WHERE conversation_id = ?1 ORDER BY id ASC",
        )?;
        let rows = stmt.query_map(params![conversation_id], |r| {
            Ok(Message {
                id: r.get(0)?,
                conversation_id: r.get(1)?,
                role: r.get(2)?,
                content: r.get(3)?,
                created_at: r.get(4)?,
            })
        })?;
        rows.collect()
    }

    pub fn add_message(
        &self,
        conversation_id: i64,
        role: &str,
        content: &str,
        embedding: Option<&[f32]>,
    ) -> rusqlite::Result<Message> {
        let t = now();
        let emb_json = embedding.map(|v| serde_json::to_string(v).unwrap_or_default());
        self.conn.execute(
            "INSERT INTO messages (conversation_id, role, content, embedding, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![conversation_id, role, content, emb_json, t],
        )?;
        let id = self.conn.last_insert_rowid();
        self.touch_conversation(conversation_id)?;
        Ok(Message {
            id,
            conversation_id,
            role: role.to_string(),
            content: content.to_string(),
            created_at: t,
        })
    }

    /// 写入一条记忆（自动或手动来源），返回新记录 id
    pub fn add_memory(
        &self,
        content: &str,
        embedding: Option<&[f32]>,
        origin: &str,
    ) -> rusqlite::Result<i64> {
        let t = now();
        let emb_json = embedding.map(|v| serde_json::to_string(v).unwrap_or_default());
        self.conn.execute(
            "INSERT INTO memories (content, embedding, origin, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![content, emb_json, origin, t, t],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn list_memories(&self) -> rusqlite::Result<Vec<MemoryRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, content, origin, created_at, updated_at FROM memories ORDER BY updated_at DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(MemoryRow {
                id: r.get(0)?,
                content: r.get(1)?,
                origin: r.get(2)?,
                created_at: r.get(3)?,
                updated_at: r.get(4)?,
            })
        })?;
        rows.collect()
    }

    pub fn update_memory(&self, id: i64, content: &str, embedding: Option<&[f32]>) -> rusqlite::Result<()> {
        let t = now();
        let emb_json = embedding.map(|v| serde_json::to_string(v).unwrap_or_default());
        self.conn.execute(
            "UPDATE memories SET content = ?1, embedding = ?2, updated_at = ?3 WHERE id = ?4",
            params![content, emb_json, t, id],
        )?;
        Ok(())
    }

    pub fn delete_memory(&self, id: i64) -> rusqlite::Result<()> {
        self.conn.execute("DELETE FROM memories WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn search_similar(&self, query: &[f32], top_k: usize, min_sim: f32) -> rusqlite::Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT content, embedding FROM memories WHERE embedding IS NOT NULL")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)))?;
        let mut scored: Vec<(f32, String)> = Vec::new();
        for row in rows {
            let (content, emb_opt) = row?;
            if let Some(emb_str) = emb_opt {
                if let Ok(emb) = serde_json::from_str::<Vec<f32>>(&emb_str) {
                    let sim = crate::memory::cosine_similarity(query, &emb);
                    if sim >= min_sim {
                        scored.push((sim, content));
                    }
                }
            }
        }
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);
        Ok(scored.into_iter().map(|(_, c)| c).collect())
    }

    /// 关键词检索（零成本兜底路径）：不依赖 embedding，也就不依赖 API Key。
    /// 用字符二元组（bigram）重合度打分，对中文友好，且无需 FTS5 分词器。
    pub fn search_keyword(&self, query: &str, top_k: usize) -> rusqlite::Result<Vec<String>> {
        let q_grams = bigrams(query);
        if q_grams.is_empty() {
            // 查询过短：直接返回最近的记忆
            let mut stmt = self
                .conn
                .prepare("SELECT content FROM memories ORDER BY updated_at DESC LIMIT ?1")?;
            let rows = stmt.query_map(params![top_k as i64], |r| r.get::<_, String>(0))?;
            return rows.collect();
        }
        let mut stmt = self.conn.prepare("SELECT content FROM memories")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut scored: Vec<(f32, String)> = Vec::new();
        for row in rows {
            let content = row?;
            let c_grams = bigrams(&content);
            if c_grams.is_empty() {
                continue;
            }
            let hit = q_grams.iter().filter(|g| c_grams.contains(g)).count();
            let sim = hit as f32 / q_grams.len() as f32;
            if sim > 0.05 {
                scored.push((sim, content));
            }
        }
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);
        Ok(scored.into_iter().map(|(_, c)| c).collect())
    }

    pub fn export_conversation(&self, conversation_id: i64) -> rusqlite::Result<String> {
        let conv = self.conn.query_row(
            "SELECT id, title, created_at, updated_at FROM conversations WHERE id = ?1",
            params![conversation_id],
            |r| {
                Ok(Conversation {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    created_at: r.get(2)?,
                    updated_at: r.get(3)?,
                })
            },
        )?;
        let messages = self.get_messages(conversation_id)?;
        let payload = serde_json::json!({
            "title": conv.title,
            "exported_at": now(),
            "messages": messages,
        });
        Ok(serde_json::to_string(&payload).unwrap_or_default())
    }

    pub fn import_conversation(&self, json: &str) -> rusqlite::Result<Conversation> {
        let v: serde_json::Value = serde_json::from_str(json)
            .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?;
        let title = v
            .get("title")
            .and_then(|t| t.as_str())
            .unwrap_or("导入的对话")
            .to_string();
        let conv = self.create_conversation(Some(title))?;
        if let Some(messages) = v.get("messages").and_then(|m| m.as_array()) {
            for m in messages {
                let role = m.get("role").and_then(|r| r.as_str()).unwrap_or("user").to_string();
                let content = m.get("content").and_then(|c| c.as_str()).unwrap_or("").to_string();
                if !content.is_empty() {
                    self.conn.execute(
                        "INSERT INTO messages (conversation_id, role, content, embedding, created_at) VALUES (?1, ?2, ?3, NULL, ?4)",
                        params![conv.id, role, content, now()],
                    )?;
                }
            }
            self.touch_conversation(conv.id)?;
        }
        Ok(conv)
    }
}
