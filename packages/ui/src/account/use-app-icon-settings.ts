import { useEffect, useState } from "react";
import type { AppIconClient, AppIconInfo } from "./account-model";
import { useAccountAction } from "./account-operation";

/** 管理图标状态、切换及失败后的系统重读，并隔离旧客户端结果。 */
export function useAppIconSettings(client: AppIconClient) {
  const [info, setInfo] = useState<AppIconInfo | null>(null);
  const [pending, setPending] = useState<string | null>(null);
  const [error, setError] = useState("");
  const { busy, mounted, clientGeneration, perform } = useAccountAction(client, setError, () => {});

  useEffect(() => {
    setInfo(null);
    setPending(null);
    setError("");
    void perform(
      async () => {
        const current = clientGeneration.current;
        try {
          const value = await client.info();
          if (mounted.current && clientGeneration.current === current) setInfo(value);
        } catch {
          if (mounted.current && clientGeneration.current === current)
            setError("暂时无法读取 App 图标状态，请稍后重试。");
        }
      },
      { allowBusy: true },
    );
  }, [client]);

  const choose = (style: string) => {
    if (!info?.supported || pending || busy || info.selected === style) return;
    const current = clientGeneration.current;
    setPending(style);
    setError("");
    void perform(async () => {
      try {
        const updated = await client.set(style);
        if (!mounted.current || clientGeneration.current !== current) return;
        setInfo(updated);
        if (updated.selected !== style) setError("图标未能更换，请稍后重试。");
      } catch {
        // Android 启动器和 iOS 模拟器可能在应用图标后仍报错，先重读系统状态再判断是否失败。
        try {
          const updated = await client.info();
          if (!mounted.current || clientGeneration.current !== current) return;
          setInfo(updated);
          if (updated.selected !== style) setError("图标未能更换，请稍后重试。");
        } catch {
          if (mounted.current && clientGeneration.current === current)
            setError("图标未能更换，请稍后重试。");
        }
      } finally {
        if (mounted.current && clientGeneration.current === current) setPending(null);
      }
    });
  };

  return { info, pending, error, choose };
}
