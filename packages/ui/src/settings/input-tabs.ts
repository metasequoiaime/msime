export const inputTabs = [
  { id: "scheme", title: "输入方案" },
  { id: "mode", title: "中英模式" },
  { id: "selection", title: "选词翻页" },
  { id: "candidates", title: "候选联想" },
  { id: "output", title: "输出设置" },
  { id: "advanced", title: "高级选项" },
] as const;

export type InputTab = (typeof inputTabs)[number]["id"];
