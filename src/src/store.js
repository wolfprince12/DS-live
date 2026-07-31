import { api } from "./api";
class Store {
    constructor() {
        this.conversations = [];
        this.currentId = null;
        this.messages = [];
        this.memories = [];
        this.model = localStorage.getItem("ds_model") || "deepseek-v4-flash";
        this.useMemory = localStorage.getItem("ds_memory") !== "0";
        this.thinking = localStorage.getItem("ds_thinking") === "1";
        this.theme = localStorage.getItem("ds_theme") || "light";
        /** "web" = 内嵌官网（免费，用自己账号）；"api" = 走 API Key（记忆注入更强） */
        this.mode = localStorage.getItem("ds_mode") || "";
    }
    setMode(m) {
        this.mode = m;
        localStorage.setItem("ds_mode", m);
    }
    async refreshConversations() {
        this.conversations = await api.getConversations();
    }
    async loadMessages(id) {
        this.currentId = id;
        this.messages = await api.getMessages(id);
    }
    setModel(m) {
        this.model = m;
        localStorage.setItem("ds_model", m);
    }
    setMemory(v) {
        this.useMemory = v;
        localStorage.setItem("ds_memory", v ? "1" : "0");
    }
    setThinking(v) {
        this.thinking = v;
        localStorage.setItem("ds_thinking", v ? "1" : "0");
    }
    setTheme(t) {
        this.theme = t;
        localStorage.setItem("ds_theme", t);
        const sysDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
        const eff = t === "system" ? (sysDark ? "dark" : "light") : t;
        document.documentElement.setAttribute("data-theme", eff);
    }
}
export const store = new Store();
