import { useEffect, useState } from "react";

export interface CustomEndpointIntentOptions {
  /** 当前地址本身是否已经需要展示自定义输入框。 */
  baseCustom: boolean;
  /** 凭据类别或服务发生变化时使用的身份键；省略表示不按身份重置。 */
  resetKey?: string;
}

export interface CustomEndpointIntentState {
  custom: boolean;
  setCustom: (custom: boolean) => void;
}

/** 共享自定义接口输入框的展开意图；用户手动展开后，地址暂时变回预设也保持展开。 */
export function useCustomEndpointIntent({
  baseCustom,
  resetKey,
}: CustomEndpointIntentOptions): CustomEndpointIntentState {
  const [requested, setRequested] = useState(false);
  useEffect(() => setRequested(false), [resetKey]);
  return {
    custom: requested || baseCustom,
    setCustom: setRequested,
  };
}
