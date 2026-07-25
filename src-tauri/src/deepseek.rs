use crate::db::Message;
use futures_util::StreamExt;
use serde_json::json;
use tauri::ipc::Channel;

const BASE: &str = "https://api.deepseek.com";
const EMBED_MODEL: &str = "deepseek-embedding";

#[derive(serde::Serialize)]
struct TokenMsg {
    t: String,
    c: String,
}

/// 调用 DeepSeek embeddings 接口，返回文本向量（维度动态读取，无所谓具体值）。
pub async fn embed(client: &reqwest::Client, api_key: &str, text: &str) -> anyhow::Result<Vec<f32>> {
    let resp = client
        .post(format!("{BASE}/embeddings"))
        .bearer_auth(api_key)
        .json(&json!({ "model": EMBED_MODEL, "input": text, "encoding_format": "float" }))
        .send()
        .await?;
    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("embeddings API 错误: {body}");
    }
    let v: serde_json::Value = resp.json().await?;
    let arr = v["data"][0]["embedding"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("embedding 响应格式异常"))?;
    let vec = arr
        .iter()
        .filter_map(|x| x.as_f64().map(|f| f as f32))
        .collect::<Vec<f32>>();
    if vec.is_empty() {
        anyhow::bail!("embedding 为空");
    }
    Ok(vec)
}

/// 流式调用对话接口，逐块通过 Channel 回传（reasoning=思考过程，answer=最终答案）。
pub async fn chat_stream(
    client: &reqwest::Client,
    api_key: &str,
    model: &str,
    system: &str,
    history: &[Message],
    user_content: &str,
    thinking: bool,
    on_token: Channel<String>,
) -> anyhow::Result<String> {
    let mut messages = vec![json!({ "role": "system", "content": system })];
    for m in history {
        messages.push(json!({ "role": m.role, "content": m.content }));
    }
    let dup = history
        .last()
        .map(|m| m.role == "user" && m.content == user_content)
        .unwrap_or(false);
    if !dup {
        messages.push(json!({ "role": "user", "content": user_content }));
    }

    let mut body = json!({
        "model": model,
        "messages": messages,
        "stream": true,
    });
    if thinking {
        body["thinking"] = json!({ "type": "enabled" });
        body["reasoning_effort"] = json!("high");
    } else {
        body["temperature"] = json!(1.0);
    }

    let resp = client
        .post(format!("{BASE}/chat/completions"))
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await?;
    if !resp.status().is_success() {
        let b = resp.text().await.unwrap_or_default();
        anyhow::bail!("对话 API 错误: {b}");
    }

    let mut stream = resp.bytes_stream();
    let mut answer_full = String::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        let text = String::from_utf8_lossy(&chunk);
        for line in text.lines() {
            let line = line.trim();
            if !line.starts_with("data:") {
                continue;
            }
            let data = line.trim_start_matches("data:").trim();
            if data == "[DONE]" {
                return Ok(answer_full);
            }
            let v: serde_json::Value = match serde_json::from_str(data) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if let Some(err) = v.get("error") {
                anyhow::bail!("流式错误: {err}");
            }
            let choice = &v["choices"][0];
            if let Some(tok) = choice["delta"]["reasoning_content"].as_str() {
                if !tok.is_empty() {
                    let msg =
                        serde_json::to_string(&TokenMsg { t: "reasoning".into(), c: tok.into() }).unwrap();
                    let _ = on_token.send(msg);
                }
            }
            if let Some(tok) = choice["delta"]["content"].as_str() {
                if !tok.is_empty() {
                    answer_full.push_str(tok);
                    let msg = serde_json::to_string(&TokenMsg { t: "answer".into(), c: tok.into() }).unwrap();
                    let _ = on_token.send(msg);
                }
            }
        }
    }
    Ok(answer_full)
}
