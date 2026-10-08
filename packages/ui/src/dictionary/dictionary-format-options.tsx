export interface DictionaryFormatOptionsProps {
  pinyin: boolean;
  rime?: boolean;
}

/** 本地与云端词库导入共用的文件格式选项。写成返回 Fragment 的普通函数，鸿蒙手机的 `SelectRow` 才能从中读出面板选项。 */
export function dictionaryFormatOptions({ pinyin, rime = false }: DictionaryFormatOptionsProps) {
  return (
    <>
      <option value="standard">词在前（标准 TSV）</option>
      <option value="windows">编码在前（Windows TSV）</option>
      {rime && <option value="rime">Rime userdb / dict.yaml</option>}
      {pinyin && <option value="hans">汉字自动注音（仅导入）</option>}
    </>
  );
}

/** 以组件形式提供同一组文件格式选项，供原生 select 使用。 */
export function DictionaryFormatOptions(props: DictionaryFormatOptionsProps) {
  return dictionaryFormatOptions(props);
}
