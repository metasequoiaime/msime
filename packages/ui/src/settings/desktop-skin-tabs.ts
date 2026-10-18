/** 内置主题和已安装皮肤共用本地入口，社区提供在线获取入口。 */
export const desktopSkinTabs = [
  { id: "mine", title: "本地皮肤" },
  { id: "community", title: "社区皮肤" },
] as const;

export type DesktopSkinTab = (typeof desktopSkinTabs)[number]["id"];
