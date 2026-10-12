import { useRef, useState } from "react";
import { GroupList } from "../core/platform-controls";
import { StatusMessage } from "../core/status-message";
import { ActionButton } from "./action-button";
import type { WubiProfile } from "./input-scheme-details-section";
import { SettingsManagerActions } from "./settings-manager-actions";
import { SettingsManagerBlock } from "./settings-manager-block";
import * as settings from "./settings-style";
import { SettingsTextareaField } from "./settings-textarea-field";

/** 一段文字（一个字或整段词）在两张五笔码表里的编码，各自短码在前；码表里没有时为空数组。 */
export interface WubiTextCodes {
  text: string;
  wubi86: string[];
  wubi98: string[];
}

/** 词库请求 `wubi_codes` 的回答（host-api `wubi_codes_json`）。 */
export interface WubiCodeLookup {
  /** 整段文字作为一个词的编码；整段不是 2 到 16 个汉字或两张表都没有它时为 null。 */
  phrase: WubiTextCodes | null;
  /** 文字里每个不同的汉字，按第一次出现的顺序。 */
  characters: WubiTextCodes[];
  /** 不同的汉字太多，后面的没有列出。 */
  truncated: boolean;
}

/** 宿主的查码入口：把文字交给词库请求 `wubi_codes`。 */
export type WubiCodeLookupClient = (text: string) => Promise<WubiCodeLookup>;

/** 输入框最多收的字符数。宿主拒绝超过 4096 字节的文字，一千个汉字（扩展区的字每个 4 字节）也在这之内。 */
export const WUBI_CODE_LOOKUP_MAX_LENGTH = 1000;

export interface WubiCodeLookupSectionProps {
  lookup: WubiCodeLookupClient;
  /** 当前选用的码表版本，结果表里标出它那一列。 */
  profile?: WubiProfile;
}

/** 五笔查码：输入或粘贴汉字，列出每个字（以及整段词）的 86 和 98 五笔编码。没有字根拆分：仓库里没有授权清楚的拆分数据。 */
export function WubiCodeLookupSection({ lookup, profile = "wubi86" }: WubiCodeLookupSectionProps) {
  const [text, setText] = useState("");
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<WubiCodeLookup | null>(null);
  const [error, setError] = useState("");
  const generation = useRef(0);
  const edit = (value: string) => {
    generation.current++;
    setText(value);
    setBusy(false);
    setResult(null);
    setError("");
  };
  const search = async () => {
    const requestGeneration = ++generation.current;
    setBusy(true);
    setError("");
    try {
      const answer = await lookup(text);
      if (generation.current === requestGeneration) setResult(answer);
    } catch {
      if (generation.current === requestGeneration) {
        setResult(null);
        setError("查码失败，请稍后重试。");
      }
    } finally {
      if (generation.current === requestGeneration) setBusy(false);
    }
  };

  const columns: [WubiProfile, string][] = [
    ["wubi86", "86 五笔"],
    ["wubi98", "98 五笔"],
  ];
  const codes = (item: WubiTextCodes, column: WubiProfile) => {
    const list = column === "wubi98" ? item.wubi98 : item.wubi86;
    return list.length > 0 ? list.join(" ") : "—";
  };
  const rows = result ? [...(result.phrase ? [result.phrase] : []), ...result.characters] : [];

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
          <ActionButton action={search} disabled={busy || !text.trim()} label="查码" />
        </SettingsManagerActions>
        {error && <StatusMessage role="alert">{error}</StatusMessage>}
        {result && rows.length === 0 && (
          <StatusMessage role="status">没有可以查的汉字。</StatusMessage>
        )}
        {rows.length > 0 && (
          <div className="overflow-x-auto">
            <table
              aria-label="五笔编码"
              className="w-full border-collapse text-sm [&_td]:border-t [&_td]:border-edge [&_td]:px-2 [&_td]:py-1.5 [&_td]:text-left [&_th]:px-2 [&_th]:pb-1.5 [&_th]:text-left [&_th]:font-medium [&_th]:text-muted"
            >
              <thead>
                <tr>
                  <th scope="col">文字</th>
                  {columns.map(([column, title]) => (
                    <th scope="col" key={column}>
                      {column === profile ? `${title}（当前）` : title}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {rows.map((item, index) => (
                  <tr key={`${index}-${item.text}`}>
                    <td className="text-body">{item.text}</td>
                    {columns.map(([column]) => (
                      <td key={column} className="font-mono">
                        {codes(item, column)}
                      </td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
        {result?.truncated && (
          <StatusMessage role="status">
            字太多，只列出了前 {result.characters.length} 个不同的字。
          </StatusMessage>
        )}
      </SettingsManagerBlock>
    </GroupList>
  );
}
