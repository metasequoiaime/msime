import { useEffect, useRef, useState, type Dispatch, type SetStateAction } from "react";
import type { Preferences } from "../core/preferences-model";
import type { HelpcodeKey } from "./helpcode-preferences";
import type { PluginClient } from "./plugin-types";
import { pluginPreferences } from "./plugin-preferences";
import { pluginErrorMessage } from "./plugin-catalog-helpers";
import { useAsyncGeneration } from "./use-async-generation";

/** 导入绑定发起时的方案族；等待系统选择器期间只在最新草稿上修改该族的包选择。 */
export function useHelpcodeImport(
  plugins: PluginClient | undefined,
  setDraft: Dispatch<SetStateAction<Preferences | undefined>>,
  refresh: () => void,
) {
  const generation = useAsyncGeneration(plugins);
  const pending = useRef<number | null>(null);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState("");
  useEffect(() => {
    pending.current = null;
    setBusy(false);
    setStatus("");
  }, [plugins]);

  const importTable = plugins?.importHelpcodeTable
    ? async (family: HelpcodeKey) => {
        const current = generation.current;
        if (pending.current === current) return;
        pending.current = current;
        setBusy(true);
        setStatus("");
        try {
          const pack = await plugins.importHelpcodeTable!();
          if (generation.current !== current || !pack) return;
          if (pack.kind !== "helpcode") throw { code: "plugin_invalid" };
          const key =
            family === "shuangpin_helpcode" ? "helpcode_pack_shuangpin" : "helpcode_pack_quanpin";
          setDraft((previous) =>
            previous
              ? { ...previous, plugins: { ...pluginPreferences(previous), [key]: pack.id } }
              : previous,
          );
          refresh();
          setStatus(
            `已导入「${pack.name}」，已选为${family === "shuangpin_helpcode" ? "双拼" : "全拼"}辅助码。`,
          );
        } catch (error) {
          if (generation.current === current) {
            setStatus(pluginErrorMessage(error, "辅助码表导入失败，请重试。"));
          }
        } finally {
          if (generation.current === current) {
            pending.current = null;
            setBusy(false);
          }
        }
      }
    : undefined;
  return { importTable, busy, status };
}
