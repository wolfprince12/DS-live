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
