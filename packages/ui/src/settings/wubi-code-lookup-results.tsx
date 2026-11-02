import { StatusMessage } from "../core/status-message";
import type { WubiCodeLookup, WubiTextCodes } from "../dictionary/dictionary-contracts";
import { knownWubiProfile, wubiProfiles, type WubiProfile } from "./wubi-preferences";

export interface WubiCodeLookupResultsProps {
  result: WubiCodeLookup | null;
  profile?: WubiProfile;
}

const columnTitles: Record<WubiProfile, string> = {
  wubi86: "86 五笔",
  wubi98: "98 五笔",
  wubixsj: "新世纪五笔",
};

/** 只展示宿主返回的三套编码、空结果及截断提示，不发起查询。三列总是都画：宿主没有回新世纪那一列（认识它之前的宿主）时显示为空。 */
export function WubiCodeLookupResults({ result, profile }: WubiCodeLookupResultsProps) {
  const current = knownWubiProfile(profile);
  const codes = (item: WubiTextCodes, column: WubiProfile) => {
    const list = item[column] ?? [];
    return list.length > 0 ? list.join(" ") : "—";
  };
  const rows = result ? [...(result.phrase ? [result.phrase] : []), ...result.characters] : [];

  return (
    <>
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
                <th scope="col" className="whitespace-nowrap">
                  文字
                </th>
                {wubiProfiles.map((column) => (
                  <th scope="col" key={column}>
                    {column === current ? `${columnTitles[column]}（当前）` : columnTitles[column]}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {rows.map((item, index) => (
                <tr key={`${index}-${item.text}`}>
                  <td className="text-body">{item.text}</td>
                  {wubiProfiles.map((column) => (
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
    </>
  );
}
