type SnapshotLine = { type?: unknown; data?: { kind?: unknown }; [key: string]: unknown };

export type QuickPhraseSnapshotFilterResult = {
  text: string;
  entries: number;
};

/** 从完整云词库快照中移除快捷短语记录，保留其他词库并让宿主重新校验摘要。 */
export async function removeQuickPhraseSnapshotRecords(
  text: string,
): Promise<QuickPhraseSnapshotFilterResult> {
  const lines = text.split("\n");
  if (lines.at(-1) === "") lines.pop();
  if (lines.length < 2) throw new Error("invalid snapshot");
  const header = JSON.parse(lines.shift()!) as SnapshotLine;
  const footer = JSON.parse(lines.pop()!) as SnapshotLine;
  if (header.type !== "header" || footer.type !== "footer") throw new Error("invalid snapshot");
  const kept: string[] = [];
  let entries = 0;
  for (const line of lines) {
    const record = JSON.parse(line) as SnapshotLine;
    const isQuick =
      (record.type === "entry" ||
        record.type === "overlay" ||
        record.type === "position" ||
        record.type === "selection") &&
      record.data?.kind === "quick";
    if (isQuick) {
      if (record.type === "entry") entries += 1;
      continue;
    }
    kept.push(line);
  }
  if (!entries) return { text, entries: 0 };
  const body = [JSON.stringify(header), ...kept].join("\n") + "\n";
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(body));
  const sha256 = Array.from(new Uint8Array(digest), (byte) =>
    byte.toString(16).padStart(2, "0"),
  ).join("");
  const nextFooter = { type: "footer", records: 1 + kept.length, sha256 };
  const result = `${body}${JSON.stringify(nextFooter)}\n`;
  return { text: result, entries };
}
