import type { Conversation, Message, Memory } from "./types";
import { api } from "./api";

class Store {
  conversations: Conversation[] = [];
  currentId: number | null = null;
  messages: Message[] = [];
  memories: Memory[] = [];
  model = localStorage.getItem("ds_model") || "deepseek-v4-flash";
  useMemory = localStorage.getItem("ds_memory") !== "0";
  thinking = localStorage.getItem("ds_thinking") === "1";
  theme = localStorage.getItem("ds_theme") || "light";

  async refreshConversations() {
    this.conversations = await api.getConversations();
  }

  async loadMessages(id: number) {
    this.currentId = id;
    this.messages = await api.getMessages(id);
  }

  setModel(m: string) {
    this.model = m;
    localStorage.setItem("ds_model", m);
  }

  setMemory(v: boolean) {
    this.useMemory = v;
    localStorage.setItem("ds_memory", v ? "1" : "0");
  }

  setThinking(v: boolean) {
    this.thinking = v;
    localStorage.setItem("ds_thinking", v ? "1" : "0");
  }

  setTheme(t: string) {
    this.theme = t;
    localStorage.setItem("ds_theme", t);
    const sysDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
    const eff = t === "system" ? (sysDark ? "dark" : "light") : t;
    document.documentElement.setAttribute("data-theme", eff);
  }
}

export const store = new Store();
