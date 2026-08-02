import { Channel } from "@tauri-apps/api/core";
import type { Conversation, Message, Memory, ApiKeyStatus, UpdateInfo } from "./types";

/**
 * 兜底 invoke：部分 Windows WebView2 上原生层只注入了 plugins、没注入 invoke，
 * 这种情况 @tauri-apps/api/core 里的 invoke 会直接 TypeError。
 * 退而求其次尝试 __TAURI__.core.invoke（需要 tauri.conf.json 配 withGlobalTauri: true），
 * 再退而求其次走老式 window.__TAURI_INTERNALS__.invoke 直接调。
 */
async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const w = window as any;
  // 1. 优先原生入口
  const direct = w.__TAURI_INTERNALS__?.invoke;
  if (typeof direct === "function") return direct(cmd, args, undefined);
  // 2. 退到 withGlobalTauri 注入的 __TAURI__.core.invoke
  const globalInvoke = w.__TAURI__?.core?.invoke;
  if (typeof globalInvoke === "function") return globalInvoke(cmd, args, undefined);
  // 3. 都没就明确报错，让上层 catch 显式提示用户（不要再静默吞）
  throw new Error(
    `Tauri IPC 不可用：__TAURI_INTERNALS__.invoke 与 __TAURI__.core.invoke 都缺失（WebView2 版本可能过旧或 Tauri IPC 注入失败）`,
  );
}

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
