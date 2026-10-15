export const expressionTabs = [
  { id: "punctuation", title: "标点" },
  { id: "language", title: "候选与释义" },
  { id: "translation", title: "候选翻译" },
  { id: "providers", title: "翻译服务" },
] as const;

export type ExpressionTab = (typeof expressionTabs)[number]["id"];
