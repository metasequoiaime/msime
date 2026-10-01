import { useEffect, useRef, useState } from "react";
import { errorCode } from "../core/error-code";
import { useMountedRef } from "./use-mounted-ref";

export interface DataDirectoryClient {
  status(): Promise<{ path: string; isDefault: boolean }>;
  pick(): Promise<string | null>;
  move(): Promise<{
    path: string;
    isDefault: boolean;
    retainedOldData: boolean;
    inputMethodRestarted?: boolean;
  }>;
}

export interface DataDirectoryConfirmOptions {
  title: string;
  message: string;
  confirmLabel: string;
}

export interface UseDataDirectoryOptions {
  client?: DataDirectoryClient;
  enabled: boolean;
  confirm: (options: DataDirectoryConfirmOptions) => Promise<boolean>;
}

/** Owns the data-directory status, move flow, and user-facing result messages. */
export function useDataDirectory({ client, enabled, confirm }: UseDataDirectoryOptions) {
  const [dataDirectory, setDataDirectory] = useState<{
    path: string;
    isDefault: boolean;
  }>();
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState("");
  const generation = useRef(0);
  const actionRunning = useRef(false);
  const mounted = useMountedRef();

  useEffect(() => {
    const current = ++generation.current;
    actionRunning.current = false;
    setBusy(false);
    if (!enabled || !client)
      return () => {
        if (generation.current === current) generation.current++;
      };
    void client
      .status()
      .then((value) => {
        if (mounted.current && generation.current === current) setDataDirectory(value);
      })
      .catch(() => {
        if (mounted.current && generation.current === current) setResult("无法读取当前数据目录。");
      });
    return () => {
      if (generation.current === current) generation.current++;
    };
  }, [client, enabled, mounted]);

  async function choose() {
    if (!client || busy || actionRunning.current) return;
    const current = generation.current;
    actionRunning.current = true;
    setBusy(true);
    setResult("");
    try {
      const target = await client.pick();
      if (generation.current !== current) return;
      if (!target) return;
      const confirmed = await confirm({
        title: "移动输入法数据？",
        message: `词库、学习记录、皮肤、剪贴板历史和设置将移动到“${target}”。移动期间输入法会短暂退出；完成后设置窗口会关闭。`,
        confirmLabel: "移动",
      });
      if (!confirmed) return;
      const moved = await client.move();
      if (generation.current !== current) return;
      setDataDirectory({ path: moved.path, isDefault: moved.isDefault });
      const restartNote =
        moved.inputMethodRestarted === false
          ? "输入法未能自动重启，请手动重启输入法后再继续输入。"
          : "";
      setResult(
        moved.retainedOldData
          ? `数据已切换到新目录；旧目录不属于水杉输入法，已为安全起见保留。${restartNote}设置窗口即将关闭。`
          : `数据已移动。${restartNote}设置窗口即将关闭，请重新打开后继续使用。`,
      );
    } catch (reason) {
      if (generation.current !== current) return;
      const code = errorCode(reason);
      setResult(
        code === "data_directory_picker_unavailable"
          ? "未找到目录选择工具，请安装 zenity 或 kdialog 后重试。"
          : code === "data_directory_not_empty"
            ? "请选择空文件夹；现有文件不会被覆盖。"
            : code === "data_directory_invalid"
              ? "该位置不能作为数据目录，请选择其他空文件夹。"
              : code === "data_directory_busy"
                ? "输入法仍在使用数据目录，请稍后重试。"
                : "移动失败，仍在使用原目录，原有数据未被删除。",
      );
    } finally {
      if (generation.current === current) {
        actionRunning.current = false;
        setBusy(false);
      }
    }
  }

  return { dataDirectory, busy, result, choose } as const;
}
