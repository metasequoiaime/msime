/**
 * 背单词卡片上的喇叭能不能读这个词头：与共享层 `vocabulary::read_aloud::english_text` 同一条规则，去掉首尾空白后只含 ASCII 字母、空格、`-`、`'`、`.`，以字母开头，不超过 64 个字符。能读时返回去掉首尾空白的那段，不能时返回 null，页面不画喇叭。
 *
 * 卡片上的词头已经在手里，不必为了决定画不画一个按钮去问宿主一次；宿主真正出声前还会按共享层的规则再核对一遍（Linux 的 `read_aloud` 命令）。
 */
export function readAloudText(word: string): string | null {
  const text = word.trim();
  return text.length <= 64 && /^[A-Za-z][A-Za-z '.-]*$/.test(text) ? text : null;
}
