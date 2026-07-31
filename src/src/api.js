import { invoke, Channel } from "@tauri-apps/api/core";
export const api = {
    hasApiKey: () => invoke("has_api_key"),
    setApiKey: (key) => invoke("set_api_key", { key }),
    getConversations: () => invoke("get_conversations"),
    createConversation: (title) => invoke("create_conversation", { title: title ?? null }),
    deleteConversation: (id) => invoke("delete_conversation", { id }),
    renameConversation: (id, title) => invoke("rename_conversation", { id, title }),
    getMessages: (id) => invoke("get_messages", { conversationId: id }),
    exportConversation: (id) => invoke("export_conversation", { conversationId: id }),
    importConversation: (json) => invoke("import_conversation", { json }),
    listMemories: () => invoke("list_memories"),
    addMemory: (content) => invoke("add_manual_memory", { content }),
    updateMemory: (id, content) => invoke("update_memory", { id, content }),
    deleteMemory: (id) => invoke("delete_memory", { id }),
    openUrl: (url) => invoke("open_url", { url }),
    openWebMode: () => invoke("open_web_mode"),
    webModeOpen: () => invoke("web_mode_open"),
    syncWebMemories: () => invoke("sync_web_memories"),
    chat: (conversationId, content, model, useMemory, thinking, onToken) => {
        const channel = new Channel();
        channel.onmessage = onToken;
        return invoke("chat", {
            conversationId,
            content,
            model,
            useMemory,
            thinking,
            onToken: channel,
        });
    },
};
