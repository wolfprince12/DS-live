export interface Conversation {
  id: number;
  title: string;
  created_at: number;
  updated_at: number;
}

export interface Message {
  id: number;
  conversation_id: number;
  role: "user" | "assistant";
  content: string;
  created_at: number;
}

export interface Memory {
  id: number;
  content: string;
  origin: "auto" | "manual" | "web";
  created_at: number;
  updated_at: number;
}

/** 启动时版本检查的结果（对应 Rust 侧 update::UpdateInfo）。 */
export interface UpdateInfo {
  /** 是否成功查到远端版本；false 表示所有源都失败，前端静默处理 */
  checked: boolean;
  has_update: boolean;
  current: string;
  latest: string;
  /** GitHub 能否直连；false 时引导用户走国内镜像 */
  github_reachable: boolean;
  /** 首选下载地址（已按可达性挑好） */
  download_url: string;
  /** 备用镜像地址 */
  mirror_urls: string[];
  notes: string;
  release_url: string;
}

export interface ApiKeyStatus {
  saved: boolean;
  /** 打码后的 Key，如 sk-abc****wxyz，仅用于让用户确认确实存住了 */
  masked: string;
  /** 是否落在系统钥匙串里（false 表示只存了本地回退文件） */
  in_keyring: boolean;
}
