import { errorCode } from "../core/error-code";
import type { LocalVoiceModel, LocalVoiceModelProgress } from "./local-models";

const LANGUAGE_NAMES: Record<string, string> = {
  zh: "中文",
  en: "英语",
  ja: "日语",
  ko: "韩语",
  yue: "粤语",
};

export function formatModelBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return "未知";
  if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(1)} GB`;
  if (bytes >= 1e6) return `${Math.round(bytes / 1e6)} MB`;
  return `${Math.max(1, Math.round(bytes / 1e3))} KB`;
}

export function localModelLanguages(languages: readonly string[]): string {
  return languages.map((language) => LANGUAGE_NAMES[language] ?? language).join("、");
}

export function localModelInUse(model: LocalVoiceModel, modelPath: string): boolean {
  const normalize = (path: string) => path.trim().replace(/[\\/]+$/, "");
  const path = normalize(modelPath);
  return path !== "" && path === normalize(model.path);
}

export function visibleLocalModels(
  models: readonly LocalVoiceModel[],
  mobile: boolean,
  modelPath: string,
): LocalVoiceModel[] {
  return models.filter(
    (model) => !mobile || !model.desktop_only || localModelInUse(model, modelPath),
  );
}

export function localModelProgressPercent(progress: LocalVoiceModelProgress | undefined): number {
  if (!progress || progress.total <= 0) return 0;
  if (progress.stage === "done") return 100;
  return Math.min(100, Math.max(0, Math.floor((progress.downloaded / progress.total) * 100)));
}

export function localModelStageLabel(stage: string): string {
  switch (stage) {
    case "download":
      return "下载中";
    case "verify":
      return "校验中";
    case "extract":
      return "解压中";
    case "done":
      return "完成";
    default:
      return "准备中";
  }
}

export function localModelErrorMessage(error: unknown): string | null {
  switch (errorCode(error)) {
    case "local_model_cancelled":
      return null;
    case "local_model_network":
      return "下载失败：无法连接下载服务器。请检查网络，或在下方填写下载镜像后保存设置再试。";
    case "local_model_http_status":
      return "下载失败：服务器拒绝了请求。请稍后重试，或更换下载镜像。";
    case "local_model_checksum_mismatch":
      return "下载的文件校验不通过，已丢弃。请重试；若使用了镜像，请确认镜像内容完整。";
    case "local_model_invalid_archive":
      return "模型文件内容不完整或不安全，已丢弃。请重试。";
    case "local_model_invalid_mirror":
      return "下载镜像地址无效，必须以 https:// 开头。";
    case "local_model_io":
      return "无法写入模型文件，请确认磁盘空间充足。";
    case "busy":
      return "该模型正在下载，请等待完成或先取消。";
    case "local_model_unknown":
      return "未知的模型。";
    case "local_model_invalid_root":
    case "unavailable":
      return "无法访问模型存放目录。";
    default:
      return "操作失败，请重试。";
  }
}

export function validModelMirror(mirror: string): boolean {
  return (
    mirror === "" ||
    (mirror.length <= 2048 &&
      mirror.length > "https://".length &&
      mirror.startsWith("https://") &&
      !/[\s\u0000-\u001f\u007f]/.test(mirror))
  );
}
