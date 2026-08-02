import { invoke, Channel } from "@tauri-apps/api/core";
import type { Conversation, Message, Memory, ApiKeyStatus, UpdateInfo } from "./types";

export const api = {
  hasApiKey: () => invoke<boolean>("has_api_key"),
  setApiKey: (key: string) => invoke<void>("set_api_key", { key }),
  apiKeyStatus: () => invoke<ApiKeyStatus>("api_key_status"),
  getConversations: () => invoke<Conversation[]>("get_conversations"),
  createConversation: (title?: string) =>
    invoke<Conversation>("create_conversation", { title: title ?? null }),
  deleteConversation: (id: number) => invoke<void>("delete_conversation", { id }),
  renameConversation: (id: number, title: string) =>
    invoke<void>("rename_conversation", { id, title }),
  getMessages: (id: number) => invoke<Message[]>("get_messages", { conversationId: id }),
  exportConversation: (id: number) =>
    invoke<string>("export_conversation", { conversationId: id }),
  importConversation: (json: string) => invoke<Conversation>("import_conversation", { json }),
  listMemories: () => invoke<Memory[]>("list_memories"),
  addMemory: (content: string) => invoke<void>("add_manual_memory", { content }),
  updateMemory: (id: number, content: string) =>
    invoke<void>("update_memory", { id, content }),
  deleteMemory: (id: number) => invoke<void>("delete_memory", { id }),
  openUrl: (url: string) => invoke<void>("open_url", { url }),
  openWebMode: () => invoke<void>("open_web_mode"),
  webModeOpen: () => invoke<boolean>("web_mode_open"),
  deactivateWebMode: () => invoke<void>("deactivate_web_mode"),
  setSuppressed: (suppressed: boolean) =>
    invoke<void>("set_webview_suppressed", { suppressed }),
  syncWebMemories: () => invoke<void>("sync_web_memories"),
  /** 启动时检查新版本（含 GitHub 可达性探测），失败不抛给用户 */
  checkUpdate: () => invoke<UpdateInfo>("check_update"),
  /** 当前安装版本号（来自 Cargo.toml），不联网 */
  getVersion: () => invoke<string>("get_version"),
  chat: (
    conversationId: number,
    content: string,
    model: string,
    useMemory: boolean,
    thinking: boolean,
    onToken: (msg: string) => void,
  ) => {
    const channel = new Channel<string>();
    channel.onmessage = onToken;
    return invoke<string>("chat", {
      conversationId,
      content,
      model,
      useMemory,
      thinking,
      onToken: channel,
    });
  },
};
