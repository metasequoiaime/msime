import { Row } from "../core/platform-controls";
import * as settings from "./settings-style";
import { SelectSettingField } from "./select-setting-field";

export type TranslationProvider = "none" | "custom" | "tencent" | "niutrans" | "account";

export interface TranslationServiceSelectorSectionProps {
  grouped?: boolean;
  available: boolean;
  provider: TranslationProvider;
  showAccountProvider: boolean;
  onChange: (provider: TranslationProvider) => void;
}

/** Translation provider choice shared by desktop hosts. */
export function TranslationServiceSelectorSection({
  grouped = false,
  available,
  provider,
  showAccountProvider,
  onChange,
}: TranslationServiceSelectorSectionProps) {
  const options = (
    <>
      <option value="none">关闭</option>
      <option value="tencent">腾讯云机器翻译</option>
      <option value="niutrans">小牛翻译（NiuTrans）</option>
      <option value="custom">自定义 DeepLX 兼容服务</option>
      {showAccountProvider && (
        <option value="account">水杉账号（候选词发送到 api.msime.app）</option>
      )}
    </>
  );

  return grouped ? (
    <div role="group" aria-label="候选词翻译服务" className={settings.rowStack}>
      <Row title="翻译服务">
        <select
          aria-label="候选词翻译服务"
          disabled={!available}
          value={provider}
          onChange={(event) => onChange(event.target.value as TranslationProvider)}
        >
          {options}
        </select>
      </Row>
    </div>
  ) : (
    <div className="section" role="group" aria-label="候选词翻译服务">
      <SelectSettingField
        label="翻译服务"
        inputLabel="候选词翻译服务"
        disabled={!available}
        value={provider}
        onChange={(value) => onChange(value as TranslationProvider)}
      >
        {options}
      </SelectSettingField>
    </div>
  );
}
