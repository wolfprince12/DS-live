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

export interface ApiKeyStatus {
  saved: boolean;
  /** 打码后的 Key，如 sk-abc****wxyz，仅用于让用户确认确实存住了 */
  masked: string;
  /** 是否落在系统钥匙串里（false 表示只存了本地回退文件） */
  in_keyring: boolean;
}
