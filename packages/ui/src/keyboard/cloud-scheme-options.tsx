/** Shared encoding choices used by the cloud dictionary catalog and candidate panels. */
export function CloudPinyinSchemeOptions() {
  return (
    <>
      <option value="pinyin">全拼</option>
      <option value="shuangpin">双拼</option>
    </>
  );
}

/** Shared shuangpin profile choices used by the cloud dictionary catalog and candidate panels. Keep this list in step with the profiles `valid_candidate_query` in crates/client-core/src/cloud/dictionary.rs accepts; a value it rejects makes every candidate request fail. */
export function CloudShuangpinProfileOptions() {
  return (
    <>
      <option value="xiaohe">小鹤</option>
      <option value="ziranma">自然码</option>
      <option value="microsoft">微软</option>
      <option value="shoudao">首道</option>
      <option value="smart_abc">智能 ABC</option>
    </>
  );
}
