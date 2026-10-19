export const aiTabs = [
  { id: "assistance", title: "辅助功能" },
  { id: "connection", title: "连接助手" },
] as const;

export type AiTab = (typeof aiTabs)[number]["id"];
