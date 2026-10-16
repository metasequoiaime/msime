import { GroupList } from "../core/platform-controls";
import { StatusMessage } from "../core/status-message";
import { ActionButton } from "./action-button";
import type { WubiProfile } from "./wubi-preferences";
import { useWubiCodeLookup } from "./use-wubi-code-lookup";
import { WubiCodeLookupResults } from "./wubi-code-lookup-results";
import { SettingsManagerActions } from "./settings-manager-actions";
import { SettingsManagerBlock } from "./settings-manager-block";
import * as settings from "./settings-style";
import { SettingsTextareaField } from "./settings-textarea-field";
import type { WubiCodeLookupClient } from "../dictionary/dictionary-contracts";
export type {
  WubiTextCodes,
  WubiCodeLookup,
  WubiCodeLookupClient,
} from "../dictionary/dictionary-contracts";

/** 输入框最多收的字符数。宿主拒绝超过 4096 字节的文字，一千个汉字（扩展区的字每个 4 字节）也在这之内。 */
export const WUBI_CODE_LOOKUP_MAX_LENGTH = 1000;

export interface WubiCodeLookupSectionProps {
  lookup: WubiCodeLookupClient;
  /** 当前选用的码表版本，结果表里标出它那一列。 */
  profile?: WubiProfile;
}

/** 五笔查码：输入或粘贴汉字，列出每个字（以及整段词）的 86 和 98 五笔编码。没有字根拆分：仓库里没有授权清楚的拆分数据。 */
export function WubiCodeLookupSection({ lookup, profile }: WubiCodeLookupSectionProps) {
  const { text, result, error, canSearch, edit, search } = useWubiCodeLookup({ lookup });

  return (
    <GroupList title="五笔查码">
      <SettingsManagerBlock>
        <SettingsTextareaField
          label="查汉字的五笔编码"
          description="输入或粘贴汉字，列出每个字的 86 和 98 五笔编码，简码在前、全码在后。打字时也能查：五笔拼音混输关闭时先按 z 再打拼音，候选后面标着它的五笔编码。"
          ariaLabel="要查五笔编码的文字"
          placeholder="例如：水杉输入法"
          maxLength={WUBI_CODE_LOOKUP_MAX_LENGTH}
          className={`${settings.fieldInput} min-h-16 w-full resize-y leading-relaxed`}
          value={text}
          onChange={edit}
        />
        <SettingsManagerActions>
          <ActionButton action={search} disabled={!canSearch} label="查码" />
        </SettingsManagerActions>
        {error && <StatusMessage role="alert">{error}</StatusMessage>}
        <WubiCodeLookupResults result={result} profile={profile} />
      </SettingsManagerBlock>
    </GroupList>
  );
}
