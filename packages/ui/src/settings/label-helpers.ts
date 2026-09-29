export function platformOsName(platform: string): string {
  switch (platform) {
    case "macos":
      return "macOS";
    case "windows":
      return "Windows";
    case "harmony":
      return "HarmonyOS";
    case "ios":
      return "iOS";
    case "android":
      return "Android";
    default:
      return "Linux";
  }
}

export function schemeTitle(scheme: string): string {
  switch (scheme) {
    case "quanpin":
      return "全拼";
    case "shuangpin":
      return "双拼";
    case "wubi":
      return "五笔";
    default:
      return "日语";
  }
}
