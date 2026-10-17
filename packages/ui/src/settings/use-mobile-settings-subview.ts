import { useState } from "react";
import { pushMobileSettingsState } from "./mobile-navigation";
import { useLatestRef } from "../core/use-latest-ref";
import { useMobilePopState } from "./use-mobile-pop-state";

export type MobileSettingsHistoryState = Record<string, unknown> | null;

export interface UseMobileSettingsSubviewOptions<Value> {
  /** 只在触屏设置宿主上监听返回手势。 */
  mobile: boolean;
  /** 当前历史记录已经表示某个子视图时的初始值；只在首次挂载时读取。 */
  initialValue?: Value | null;
  /** 当前子视图在历史记录里使用的字段名，用于关闭时确认记录归属。 */
  stateKey: string;
  /** 把历史记录中的路由字段解码成子视图值；不匹配时返回 null。 */
  decode: (state: MobileSettingsHistoryState, currentValue: Value | null) => Value | null;
  /** 把子视图值编码成要覆盖当前设置历史记录的字段。 */
  encode: (value: Value) => Record<string, unknown>;
  /** 某条历史记录是否属于这个子视图；缺省检查 `stateKey` 存在且不是 undefined。 */
  isActiveState?: (state: MobileSettingsHistoryState) => boolean;
}

export interface MobileSettingsSubviewState<Value> {
  value: Value | null;
  open: (value: Value) => void;
  close: () => void;
  reset: () => void;
}

function asHistoryState(value: unknown): MobileSettingsHistoryState {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : null;
}

/** 共用触屏设置页的子视图历史动作；页面字段与子视图内容仍由调用方持有。 */
export function useMobileSettingsSubview<Value>({
  mobile,
  initialValue,
  stateKey,
  decode,
  encode,
  isActiveState,
}: UseMobileSettingsSubviewOptions<Value>): MobileSettingsSubviewState<Value> {
  const [value, setValue] = useState<Value | null>(initialValue ?? null);
  const decodeRef = useLatestRef(decode);
  const encodeRef = useLatestRef(encode);
  const activeStateRef = useLatestRef(
    isActiveState ?? ((state: MobileSettingsHistoryState) => state?.[stateKey] !== undefined),
  );

  useMobilePopState(mobile, (event) => {
    setValue(decodeRef.current(asHistoryState(event.state), value));
  });

  const open = (next: Value) => {
    if (mobile && typeof window !== "undefined") {
      pushMobileSettingsState(encodeRef.current(next));
    }
    setValue(next);
  };

  const close = () => {
    if (
      mobile &&
      typeof window !== "undefined" &&
      activeStateRef.current(asHistoryState(window.history.state))
    ) {
      window.history.back();
      return;
    }
    setValue(null);
  };

  return { value, open, close, reset: () => setValue(null) };
}
