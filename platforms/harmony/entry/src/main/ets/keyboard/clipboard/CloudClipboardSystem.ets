import pasteboard from '@ohos.pasteboard';

/** 空串仅表示空剪贴板；非文本和读取失败不能被当作空白覆盖。 */
export async function cloudClipboardSystemText(): Promise<string | null> {
  try {
    const data: pasteboard.PasteData = await pasteboard.getSystemPasteboard().getData();
    if (data.getRecordCount() === 0) return '';
    if (!data.hasType(pasteboard.MIMETYPE_TEXT_PLAIN)) return null;
    const text: string | undefined = data.getPrimaryText();
    return text === undefined ? null : text;
  } catch (_) {
    return null;
  }
}
