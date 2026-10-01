export interface DictionaryFormatOptionsProps {
  pinyin: boolean;
  rime?: boolean;
}

/** Shared file format choices used by local and cloud dictionary importers. */
export function DictionaryFormatOptions({ pinyin, rime = false }: DictionaryFormatOptionsProps) {
  return (
    <>
      <option value="standard">词在前（标准 TSV）</option>
      <option value="windows">编码在前（Windows TSV）</option>
      {rime && <option value="rime">Rime userdb / dict.yaml</option>}
      {pinyin && <option value="hans">汉字自动注音（仅导入）</option>}
    </>
  );
}
