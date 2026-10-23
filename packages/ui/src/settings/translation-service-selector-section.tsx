import { SettingsRowStack } from "./settings-row-stack";
import { SelectSettingField } from "./select-setting-field";
import { SelectRow } from "./select-row";

export type TranslationProvider = "none" | "custom" | "tencent" | "niutrans" | "account" | "ai";

/** 不选在线服务时宿主还会用什么出释义，决定 `none` 一项的叫法。`system`：macOS，离线词库，macOS 26 起再加系统本机翻译（`InputController` 的 `currentOnDeviceGlossRequest`）。`dictionary`：Windows、Linux、HarmonyOS，没有系统翻译，但 host-api 仍把装好的离线词库语言放进查询（`msime_client_translation_query` 的 `offline_gloss_languages`），各宿主照样查出释义。缺省（Android、iOS）沿用「关闭」，那边没有核对过。 */
export type TranslationOfflineFallback = "system" | "dictionary";

/** 各宿主的 `TranslationOfflineFallback`。 */
export function translationOfflineFallback({
  macos,
  windows = false,
  linux,
  harmony = false,
}: {
  macos: boolean;
  windows?: boolean;
  linux: boolean;
  harmony?: boolean;
}): TranslationOfflineFallback | undefined {
  if (macos) return "system";
  if (windows || linux || harmony) return "dictionary";
  return undefined;
}

export interface TranslationServiceSelectorSectionProps {
  grouped?: boolean;
  available: boolean;
  provider: TranslationProvider;
  showAccountProvider: boolean;
  /** 不选在线服务时宿主仍会出的释义（见 `TranslationOfflineFallback`）。有值时 `none` 不再叫「关闭」，因为候选词翻译打开时它并没有关掉释义：`system` 叫「离线翻译（不联网）」，`dictionary` 叫「不使用在线服务」，两者都说明释义从哪里来。 */
  offlineFallback?: TranslationOfflineFallback;
  onChange: (provider: TranslationProvider) => void;
}

const offlineFallbackCopy: Record<
  TranslationOfflineFallback,
  { label: string; description: string }
> = {
  system: {
    label: "离线翻译（不联网）",
    description:
      "离线翻译用随输入法打包的离线词库，macOS 26 起词库查不到的候选再由系统在本机翻译，都不联网；选择在线服务后由该服务翻译",
  },
  dictionary: {
    label: "不使用在线服务",
    description:
      "不联网，候选只显示离线词库查得到的释义：英文来自「显示英文释义」，其他语言要装有对应的离线词库；选择在线服务后由该服务翻译",
  },
};

/** 翻译服务的选项。写成返回 Fragment 的普通函数而不是组件，鸿蒙手机的 `SelectRow` 才能从中读出面板选项。 */
function translationServiceOptions(
  showAccountProvider: boolean,
  offlineFallback: TranslationOfflineFallback | undefined,
) {
  return (
    <>
      <option value="none">
        {offlineFallback ? offlineFallbackCopy[offlineFallback].label : "关闭"}
      </option>
      <option value="tencent">腾讯云机器翻译</option>
      <option value="niutrans">小牛翻译（NiuTrans）</option>
      <option value="custom">自定义（DeepLX 或 DeepL API）</option>
      <option value="ai">AI 辅助</option>
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
  offlineFallback,
  onChange,
}: TranslationServiceSelectorSectionProps) {
  // 选中 AI 辅助时换成复用说明与成本提示（与 Windows 设置页「复用 AI 辅助配置」一致）；
  // AI 模型逐条生成释义，按 token 计费且比专用翻译接口慢。
  const description =
    provider === "ai"
      ? "复用「AI 与云联想」页当前服务商的接口地址、Token 与模型，无需在此重复填写；AI 模型逐条生成释义，成本与延迟高于专用翻译服务"
      : offlineFallback
        ? offlineFallbackCopy[offlineFallback].description
        : undefined;
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
