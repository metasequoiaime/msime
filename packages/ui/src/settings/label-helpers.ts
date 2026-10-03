import type { InputScheme } from "../index";

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

export function schemeTitle(scheme: InputScheme): string {
  switch (scheme) {
    case "quanpin":
      return "全拼";
    case "shuangpin":
      return "双拼";
    case "wubi":
      return "五笔";
    case "japanese":
      return "日语";
    case "korean":
      return "韩语";
    case "cantonese":
      return "粤拼";
    case "zhuyin":
      return "注音";
    case "vietnamese":
      return "越南语";
    case "tibetan":
      return "藏文";
    default: {
      const unhandled: never = scheme;
      return unhandled;
    }
  }
}
