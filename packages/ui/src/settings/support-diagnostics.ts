import { platformOsName } from "./label-helpers";

export interface SupportDiagnosticsHost {
  platform: string;
  os_version?: string;
}

export interface SupportDiagnosticsOptions {
  version: string;
  host?: SupportDiagnosticsHost;
  scheme?: string;
  fallbackPlatform: string;
  userAgent?: string;
}

/** Builds the compact diagnostic text attached to support reports. */
export function supportDiagnostics({
  version,
  host,
  scheme,
  fallbackPlatform,
  userAgent,
}: SupportDiagnosticsOptions) {
  return [
    `水杉 IME ${version}`,
    host?.os_version
      ? `${platformOsName(host.platform)} ${host.os_version}`
      : `平台：${host?.platform ?? fallbackPlatform}`,
    scheme ? `输入方案：${scheme}` : "",
    host?.os_version || !userAgent ? "" : `User-Agent：${userAgent.slice(0, 256)}`,
  ]
    .filter(Boolean)
    .join("\n");
}
