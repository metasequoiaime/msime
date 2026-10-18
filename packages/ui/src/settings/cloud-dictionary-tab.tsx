import { useMemo, useState } from "react";
import {
  CloudCandidatesPanel,
  CloudDictionaryApplyPanel,
  CloudDictionaryCatalogPanel,
  CloudDictionaryFilesPanel,
  CloudDictionaryPanel,
} from "../keyboard/panels";
import type { CloudDictionaryPanelClient } from "../keyboard/panel-client";

type CloudDictionaryView = "dictionary" | "catalog" | "candidates" | "files" | "apply";

/** 词库页里的云词库标签；云词库的目录和备份子页仍在同一个设置窗口内切换。 */
export function CloudDictionaryTab({
  client,
  onClose,
}: {
  client: CloudDictionaryPanelClient;
  onClose: () => void;
}) {
  const [view, setView] = useState<CloudDictionaryView>("dictionary");
  const close = useMemo(() => async () => onClose(), [onClose]);
  const base = useMemo(() => ({ ...client, close }), [client, close]);
  let content;
  if (view === "catalog")
    content = (
      <CloudDictionaryCatalogPanel client={{ ...base, back: async () => setView("dictionary") }} />
    );
  else if (view === "candidates")
    content = (
      <CloudCandidatesPanel client={{ ...base, back: async () => setView("dictionary") }} />
    );
  else if (view === "files")
    content = (
      <CloudDictionaryFilesPanel client={{ ...base, back: async () => setView("dictionary") }} />
    );
  else if (view === "apply")
    content = (
      <CloudDictionaryApplyPanel client={{ ...base, back: async () => setView("dictionary") }} />
    );
  else
    content = (
      <CloudDictionaryPanel
        embedded
        client={{
          ...base,
          openCatalog: async () => setView("catalog"),
          openCandidates: async () => setView("candidates"),
          openFiles: async () => setView("files"),
          openApply: async () => setView("apply"),
        }}
      />
    );
  return (
    <>
      {view !== "dictionary" && (
        <div className="border-b border-edge px-5 py-3">
          <button type="button" className="secondary mt-0" onClick={() => setView("dictionary")}>
            返回云词库
          </button>
        </div>
      )}
      {content}
    </>
  );
}
