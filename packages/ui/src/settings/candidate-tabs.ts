export const candidateTabs = [
  { id: "layout", title: "布局" },
  { id: "font", title: "字体与大小" },
  { id: "style", title: "窗口样式" },
  { id: "preedit", title: "预编辑" },
] as const;

export type CandidateTab = (typeof candidateTabs)[number]["id"];
