import { invoke, Channel } from "@tauri-apps/api/core";
import type { Conversation, Message, Memory } from "./types";

export const api = {
  hasApiKey: () => invoke<boolean>("has_api_key"),
  setApiKey: (key: string) => invoke<void>("set_api_key", { key }),
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
