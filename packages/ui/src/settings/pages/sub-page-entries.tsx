import { GroupList, LinkRow } from "../../core/platform-controls";
import type { SettingsFormModel } from "../settings-form-context";
import { useSettingsForm } from "../settings-form-context";

type PageId = Parameters<SettingsFormModel["selectPage"]>[0];

/** 打开本页内嵌页面的行（「AI 辅助」上的「AI 对话」、「词库」上的「背单词」、「帮助与反馈」上的「帮助」）。每一行以它打开的页面命名；宿主不提供的页面没有对应的行。 */
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
