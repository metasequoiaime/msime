export const expressionTabs = [
  { id: "punctuation", title: "标点设置" },
  // 「显示英文释义」和「候选词翻译」画的是候选后面同一行释义，目标语言也是两者共用，所以放在同一个标签里。
  { id: "gloss", title: "候选释义" },
  { id: "providers", title: "翻译服务" },
] as const;

export type ExpressionTab = (typeof expressionTabs)[number]["id"];
