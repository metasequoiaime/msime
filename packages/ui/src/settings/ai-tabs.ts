export const aiTabs = [
  { id: "assistance", title: "输入辅助" },
  { id: "connection", title: "连接助手" },
] as const;

export type AiTab = (typeof aiTabs)[number]["id"];
