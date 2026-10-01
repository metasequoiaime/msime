import { GroupList, LinkRow } from "../../core/platform-controls";
import type { SettingsFormModel } from "../settings-form-context";
import { useSettingsForm } from "../settings-form-context";

type PageId = Parameters<SettingsFormModel["selectPage"]>[0];

/** Rows that open the pages living inside this one (AI 辅助 on 表达, 背单词 on 词库, 帮助 on 反馈). Each row is named by the page it opens; a page the host does not offer has no row. */
export function SubPageEntries({
  title,
  pages,
}: {
  title: string;
  pages: readonly { id: PageId; description: string }[];
}) {
  const { pageEntry, selectPage } = useSettingsForm();
  const entries = pages.flatMap((item) => {
    const entry = pageEntry(item.id);
    return entry ? [{ ...item, title: entry.title }] : [];
  });
  if (entries.length === 0) return null;
  return (
    <GroupList title={title}>
      {entries.map((entry) => (
        <LinkRow
          key={entry.id}
          title={entry.title}
          description={entry.description}
          onClick={() => selectPage(entry.id)}
        />
      ))}
    </GroupList>
  );
}
