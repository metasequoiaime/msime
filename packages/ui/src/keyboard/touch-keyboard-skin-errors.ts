import { errorCode } from "../core/error-code";

export function aiSkinMessage(error: unknown): string {
  switch (errorCode(error)) {
    case "ai_skin_invalid":
      return "AI 返回的皮肤设计或插画格式无效，请重新抽取。";
    case "ai_skin_cancelled":
      return "已取消这次抽卡。";
    case "ai_skin_busy":
      return "已有一次抽卡正在进行，请稍候。";
    case "account_unauthorized":
      return "请先登录，再来抽取皮肤。";
    case "account_rate_limited":
      return "请求过于频繁，请稍后再试。";
    default:
      return "AI 皮肤暂时不可用，请稍后重试。";
  }
}

export function libraryError(error: unknown): string {
  switch (errorCode(error)) {
    case "custom_skin_full":
      return "最多保存 12 套皮肤，请先删除不需要的设计。";
    case "custom_skin_invalid_name":
      return "请输入皮肤名称。";
    case "custom_skin_duplicate_name":
      return "已经有同名皮肤，请换一个名称。";
    case "custom_skin_not_found":
      return "这套皮肤已在其他窗口中变更，请重新打开图库。";
    case "custom_skin_format":
      return "皮肤图库无法读取，原文件已保留。";
    default:
      return "皮肤图库保存失败，请检查设备可用空间后重试。";
  }
}
