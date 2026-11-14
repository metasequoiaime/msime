import { expect, test } from "vitest";
import { removeQuickPhraseSnapshotRecords } from "../../../../packages/ui/src/keyboard/cloud-quick-phrase-snapshot";

test("云端快照过滤只移除快捷短语记录并重算 footer", async () => {
  const text = [
    '{"type":"header","format":"msime-dictionary-snapshot","version":1,"revision":7}',
    '{"type":"entry","data":{"kind":"pinyin","code":"nihao","word":"你好"}}',
    '{"type":"entry","data":{"kind":"quick","code":"a","word":"合成"}}',
    '{"type":"overlay","deleted":false,"data":{"kind":"quick","code":"a","word":"合成"}}',
    '{"type":"footer","records":4,"sha256":"synthetic"}',
    "",
  ].join("\n");

  const result = await removeQuickPhraseSnapshotRecords(text);
  const lines = result.text.trimEnd().split("\n");
  const footer = JSON.parse(lines.at(-1)!);

  expect(result.entries).toBe(1);
  expect(result.revision).toBe(7);
  expect(result.text).toContain('"kind":"pinyin"');
  expect(result.text).not.toContain('"kind":"quick"');
  expect(footer.records).toBe(2);
  expect(footer.sha256).toMatch(/^[0-9a-f]{64}$/);
});

test("没有快捷短语时不改动云端快照文本", async () => {
  const text = [
    '{"type":"header","format":"msime-dictionary-snapshot","version":1,"revision":7}',
    '{"type":"footer","records":1,"sha256":"synthetic"}',
    "",
  ].join("\n");

  await expect(removeQuickPhraseSnapshotRecords(text)).resolves.toEqual({
    text,
    entries: 0,
    revision: 7,
  });
});
