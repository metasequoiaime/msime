export const inputTabs = [
  { id: "scheme", title: "方案" },
  { id: "mode", title: "中英文" },
  { id: "selection", title: "选词与翻页" },
  { id: "candidates", title: "候选与联想" },
  { id: "output", title: "输出" },
  { id: "advanced", title: "进阶" },
] as const;

export type InputTab = (typeof inputTabs)[number]["id"];
