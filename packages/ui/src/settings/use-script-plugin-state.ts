import { useEffect, useRef, useState } from "react";
import { pluginErrorCode, pluginErrorMessage } from "./plugin-catalog-helpers";
import { settingErrorKey } from "./plugin-script-model";
import type { PluginClient, ScriptPluginState, ScriptSettingValue } from "./plugin-types";

/** 宿主里读写脚本插件设置值和持久存储的三项；宿主没有时详情页不显示设置表单和已存数据。 */
export type ScriptPluginStateClient = Pick<
  PluginClient,
  "scriptPluginState" | "setScriptSettings" | "clearScriptData"
>;

/** 保存设置失败的原因：`key` 是宿主说出了是哪一项（`plugin_script_setting_invalid`），否则为 null，说明放在表单底部。 */
export type ScriptSettingsFailure = { key: string | null; message: string };

/**
 * 插件详情页里一个脚本插件的设置值和存储大小。`revision` 是授权文件的版本：授权一变（撤销 `persist_data` 会删掉存储）就重读。保存和清除失败时抛出，由调用方说明。
 */
export function useScriptPluginState(
  client: ScriptPluginStateClient | undefined,
  id: string,
  contentHash: string | undefined,
  revision: number | undefined,
) {
  const [state, setState] = useState<ScriptPluginState | null>(null);
  const [loadError, setLoadError] = useState("");
  // 换了插件或重读之后，晚到的旧回答不能盖掉新的。
  const generation = useRef(0);
  const read = client?.scriptPluginState;

  useEffect(() => {
    if (!read) return;
    const current = ++generation.current;
    read(id)
      .then((next) => {
        if (current !== generation.current) return;
        setState(next);
        setLoadError("");
      })
      .catch((error: unknown) => {
        if (current === generation.current)
          setLoadError(pluginErrorMessage(error, "无法读取插件的设置和数据，请重试。"));
      });
  }, [read, id, contentHash, revision]);

  const saveSettings = async (values: Record<string, ScriptSettingValue>) => {
    const current = ++generation.current;
    try {
      const next = await client!.setScriptSettings!(id, values);
      if (current === generation.current) setState(next);
    } catch (error) {
      const detail =
        typeof error === "object" && error !== null && "detail" in error
          ? (error as { detail: unknown }).detail
          : null;
      const failure: ScriptSettingsFailure = {
        key:
          pluginErrorCode(error) === "plugin_script_setting_invalid"
            ? settingErrorKey(detail)
            : null,
        message: pluginErrorMessage(error, "设置没有保存，请重试。"),
      };
      throw failure;
    }
  };

  const clearData = async () => {
    const current = ++generation.current;
    const next = await client!.clearScriptData!(id);
    if (current === generation.current) setState(next);
  };

  return {
    state,
    loadError,
    canSave: Boolean(client?.setScriptSettings),
    canClear: Boolean(client?.clearScriptData),
    saveSettings,
    clearData,
  };
}
