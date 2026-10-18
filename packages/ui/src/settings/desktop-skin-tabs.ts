/** 桌面主题与两种皮肤来源共用同一层导航。 */
export const desktopSkinTabs = [
  { id: "mine", title: "主题" },
  { id: "local", title: "本地皮肤" },
  { id: "community", title: "社区皮肤" },
] as const;

export type DesktopSkinTab = (typeof desktopSkinTabs)[number]["id"];
