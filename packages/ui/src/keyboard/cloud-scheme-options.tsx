/** Shared encoding choices used by the cloud dictionary catalog and candidate panels. */
export function CloudPinyinSchemeOptions() {
  return (
    <>
      <option value="pinyin">全拼</option>
      <option value="shuangpin">双拼</option>
    </>
  );
}

/** Shared shuangpin profile choices used by the cloud dictionary catalog and candidate panels. */
export function CloudShuangpinProfileOptions() {
  return (
    <>
      <option value="xiaohe">小鹤</option>
      <option value="ziranma">自然码</option>
      <option value="microsoft">微软</option>
      <option value="shoudao">首道</option>
    </>
  );
}
