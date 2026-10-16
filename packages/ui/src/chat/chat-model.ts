export type ChatMessage = {
  role: "user" | "assistant" | "system";
  content: string;
};

export type ChatModel = { id: string };

export type ChatModels = {
  data: ChatModel[];
  defaultModel: string;
};

export interface ChatClient {
  models(): Promise<ChatModels>;
  complete(messages: ChatMessage[], model: string): Promise<string>;
}
