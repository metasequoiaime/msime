/** 内置主题和已安装皮肤共用本地入口；「制作皮肤」紧跟在它后面，做好的皮肤回到本地入口里；社区提供在线获取入口，没有社区客户端时不出现，制作入口的位置因此不变。 */
export const desktopSkinTabs = [
  { id: "mine", title: "本地皮肤" },
  { id: "create", title: "制作皮肤" },
  { id: "community", title: "社区皮肤" },
] as const;

export type DesktopSkinTab = (typeof desktopSkinTabs)[number]["id"];
