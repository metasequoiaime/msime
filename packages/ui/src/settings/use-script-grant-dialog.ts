import { useState } from "react";
import { pluginErrorMessage } from "./plugin-catalog-helpers";
import { sortCapabilities, toggleCapability } from "./plugin-script-model";
import type { PluginPackage, ScriptCapability } from "./plugin-types";

interface ScriptGrantDialogOptions {
  pack: PluginPackage;
  initial: readonly ScriptCapability[];
  onConfirm: (granted: ScriptCapability[]) => Promise<void>;
}

/** 保留授权编辑，只按当前清单投影勾选和提交的能力；写入失败后允许修改并重试。 */
export function useScriptGrantDialog({ pack, initial, onConfirm }: ScriptGrantDialogOptions) {
  const requested = sortCapabilities(pack.permissions ?? []);
  const [granted, setGranted] = useState<ScriptCapability[]>(() =>
    sortCapabilities(initial.filter((item) => requested.includes(item))),
  );
  // 现在列着、勾着的那几项。包在对话框打开之后被换过（`plugin_script_changed`，页面重读后 `pack` 换成新版本）时，`granted` 里可能还留着新版本不再申请的一项：它不显示，不能算作勾着去禁用互斥的另一项，也不能被交出去、被宿主以「清单没有申请」拒绝。
  const selected = granted.filter((item) => requested.includes(item));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const confirm = async () => {
    setBusy(true);
    setError("");
    try {
      await onConfirm(selected);
    } catch (failure) {
      setError(pluginErrorMessage(failure, "授权没有写入，请重试。"));
      setBusy(false);
    }
  };
  const setCapability = (capability: ScriptCapability, checked: boolean) =>
    setGranted((current) => toggleCapability(current, capability, checked, requested));

  return { requested, selected, busy, error, setCapability, confirm };
}
