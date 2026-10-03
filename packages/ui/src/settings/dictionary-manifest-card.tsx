import { SettingsGroupNote } from "./settings-group-note";
import { useEffect, useState } from "react";
import { GroupList } from "../core/platform-controls";
import { SummaryRow } from "./summary-row";

export interface DictionaryManifest {
  profile: string;
  sourceCommit: string;
}

export interface DictionaryManifestCardProps {
  read: () => Promise<DictionaryManifest>;
}

/** Shows the packaged dictionary's profile and source revision when available. */
export function DictionaryManifestCard({ read }: DictionaryManifestCardProps) {
  const [manifest, setManifest] = useState<DictionaryManifest | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let active = true;
    void read()
      .then((value) => {
        if (active) setManifest(value);
      })
      .catch(() => {
        if (active) setFailed(true);
      });
    return () => {
      active = false;
    };
  }, [read]);

  if (failed) {
    return (
      <GroupList title="词库信息">
        <SettingsGroupNote>无法读取随应用安装的词库清单，请重新安装后再试。</SettingsGroupNote>
      </GroupList>
    );
  }
  if (!manifest) return null;
  return (
    <GroupList title="词库信息">
      <SettingsGroupNote>
        词库保存在设备上，日常输入不需要联网；它随应用更新，不单独下载。
      </SettingsGroupNote>
      <SummaryRow title="规格">
        <code>{manifest.profile}</code>
      </SummaryRow>
      {/* Twelve characters is what the source shows: enough to identify the build, short enough to read back over the phone. */}
      <SummaryRow title="词库版本">
        <code>{manifest.sourceCommit.slice(0, 12)}</code>
      </SummaryRow>
    </GroupList>
  );
}
