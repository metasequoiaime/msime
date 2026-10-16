import { StatusMessage } from "../core/status-message";
import type { WubiCodeLookup, WubiTextCodes } from "../dictionary/dictionary-contracts";
import type { WubiProfile } from "./wubi-preferences";

export interface WubiCodeLookupResultsProps {
  result: WubiCodeLookup | null;
  profile?: WubiProfile;
}

/** 只展示宿主返回的两套编码、空结果及截断提示，不发起查询。 */
export function WubiCodeLookupResults({ result, profile = "wubi86" }: WubiCodeLookupResultsProps) {
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
    </>
  );
}
