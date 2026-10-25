import { useEffect, useRef, useState, type ReactNode } from "react";
import { ActionButton } from "./action-button";
import { compactPage } from "../core/page-style";

/** 移动宿主提供系统认证；只有明确未配置凭据或验证成功时才完成 Promise。 */
export interface StatisticsAuthenticationClient {
  /** 只在提供本机开关的宿主显示「我的」设置行。 */
  settings?: StatisticsAuthenticationSettings;
  authenticate(): Promise<void>;
  cancel(): void;
  onLock(listener: () => void): () => void;
  onResume(listener: () => void): () => void;
}

/** 授权前不挂载内容，阻止统计读取和上传控件的副作用。 */
export function StatisticsAuthenticationGate({
  client,
  children,
}: {
  client: StatisticsAuthenticationClient;
  children: ReactNode;
}) {
  const [authorizedClient, setAuthorizedClient] = useState<StatisticsAuthenticationClient | null>(
    null,
  );
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [visible, setVisible] = useState(document.visibilityState !== "hidden");
  const generation = useRef(0);
  const pending = useRef(false);
  const attempted = useRef(false);

  function authenticate() {
    if (pending.current) return;
    const request = ++generation.current;
    attempted.current = true;
    pending.current = true;
    setBusy(true);
    setMessage("");
    void Promise.resolve()
      .then(() => {
        if (request !== generation.current) throw new Error("认证请求已失效");
        return client.authenticate();
      })
      .then(
        () => {
          if (request === generation.current) setAuthorizedClient(client);
        },
        () => {
          if (request === generation.current) setMessage("未完成身份验证，请重试。");
        },
      )
      .finally(() => {
        if (request !== generation.current) return;
        pending.current = false;
        setBusy(false);
      });
  }

  useEffect(() => {
    function lock() {
      generation.current++;
      pending.current = false;
      attempted.current = false;
      setAuthorizedClient(null);
      setBusy(false);
      setMessage("");
      client.cancel();
    }
    function visibilityChanged() {
      const visible = document.visibilityState !== "hidden";
      setVisible(visible);
      // 系统认证窗口本身可能临时遮住网页；真正退后台由原生宿主发送锁定事件。
      if (visible && !attempted.current) authenticate();
    }
    const unsubscribe = client.onLock(lock);
    const unsubscribeResume = client.onResume(visibilityChanged);
    document.addEventListener("visibilitychange", visibilityChanged);
    if (document.visibilityState !== "hidden") authenticate();
    return () => {
      generation.current++;
      pending.current = false;
      attempted.current = false;
      client.cancel();
      unsubscribe();
      unsubscribeResume();
      document.removeEventListener("visibilitychange", visibilityChanged);
    };
  }, [client]);

  if (authorizedClient === client && visible) return children;
  return (
    <div className={compactPage}>
      <p role="status">{message || "验证身份后查看打字统计"}</p>
      <ActionButton disabled={busy} action={authenticate} label={busy ? "正在验证…" : "验证身份"} />
    </div>
  );
}

/** 宿主持久化本机认证选择，不进入账号偏好或同步文档。 */
export interface StatisticsAuthenticationSettings {
  load(): Promise<boolean>;
  save(enabled: boolean): Promise<void>;
}
