import { SettingsRowStack } from "./settings-row-stack";
import { SelectSettingField } from "./select-setting-field";
import { SelectRow } from "./select-row";

export type TranslationProvider = "none" | "custom" | "tencent" | "niutrans" | "account";

export interface TranslationServiceSelectorSectionProps {
  grouped?: boolean;
  available: boolean;
  provider: TranslationProvider;
  showAccountProvider: boolean;
  /** 不选在线服务时宿主仍用离线翻译（macOS：离线词库，macOS 26 起再加系统本机翻译，见 `InputController` 的 `currentOnDeviceGlossRequest`）。为真时 `none` 叫「离线翻译」并说明来源，不再叫「关闭」，因为候选词翻译打开时它并没有关掉翻译。 */
  offlineFallback?: boolean;
  onChange: (provider: TranslationProvider) => void;
}

const offlineFallbackDescription =
  "离线翻译用随输入法打包的离线词库，macOS 26 起词库查不到的候选再由系统在本机翻译，都不联网；选择在线服务后由该服务翻译";

/** 翻译服务的选项。写成返回 Fragment 的普通函数而不是组件，鸿蒙手机的 `SelectRow` 才能从中读出面板选项。 */
function translationServiceOptions(showAccountProvider: boolean, offlineFallback: boolean) {
  return (
    <>
      <option value="none">{offlineFallback ? "离线翻译（不联网）" : "关闭"}</option>
      <option value="tencent">腾讯云机器翻译</option>
      <option value="niutrans">小牛翻译（NiuTrans）</option>
      <option value="custom">自定义（DeepLX 或 DeepL API）</option>
      {showAccountProvider && (
        <option value="account">水杉账号（候选词发送到 api.msime.app）</option>
      )}
    </>
  );
}

/** Translation provider choice shared by desktop hosts. */
export function TranslationServiceSelectorSection({
  grouped = false,
  available,
  provider,
  showAccountProvider,
  offlineFallback = false,
  onChange,
}: TranslationServiceSelectorSectionProps) {
  const description = offlineFallback ? offlineFallbackDescription : undefined;
  return grouped ? (
    <SettingsRowStack role="group" aria-label="候选词翻译服务">
      <SelectRow
        title="翻译服务"
        description={description}
        aria-label="候选词翻译服务"
        disabled={!available}
        value={provider}
        onChange={(event) => onChange(event.target.value as TranslationProvider)}
      >
        {translationServiceOptions(showAccountProvider, offlineFallback)}
      </SelectRow>
    </SettingsRowStack>
  ) : (
    <div className="section" role="group" aria-label="候选词翻译服务">
      <SelectSettingField
        label="翻译服务"
        inputLabel="候选词翻译服务"
        description={description}
        disabled={!available}
        value={provider}
        onChange={(value) => onChange(value as TranslationProvider)}
      >
        {translationServiceOptions(showAccountProvider, offlineFallback)}
      </SelectSettingField>
    </div>
  );
}
