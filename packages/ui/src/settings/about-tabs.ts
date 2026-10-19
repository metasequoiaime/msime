export const aboutTabs = [
  { id: "version", title: "版本更新" },
  { id: "feedback", title: "帮助与反馈" },
  { id: "system", title: "系统信息" },
  { id: "privacy", title: "许可与隐私" },
] as const;

export type AboutTab = (typeof aboutTabs)[number]["id"];
