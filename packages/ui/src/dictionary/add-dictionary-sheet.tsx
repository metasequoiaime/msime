import { useRef } from "react";
import { ActionSheet } from "../core/action-sheet";
import type { CollectionImportSource } from "./dictionary-collections";

/** 「添加词库」：底部面板里先是「新建空词库」，再是各种导入来源；选了来源就打开系统的文件选择器，导入的词条成为一个新词库。 */
export function AddDictionarySheet({
  open,
  sources,
  onClose,
  onCreate,
  onFile,
}: {
  open: boolean;
  sources: CollectionImportSource[];
  onClose: () => void;
  onCreate: () => void;
  onFile: (source: CollectionImportSource, file: File) => void;
}) {
  const input = useRef<HTMLInputElement>(null);
  const chosen = useRef<CollectionImportSource | null>(null);
  return (
    <>
      <ActionSheet
        open={open}
        title="添加词库"
        subtitle="导入的词条会成为一个新的词库"
        options={[
          { value: "create", label: "新建空词库" },
          ...sources.map((source) => ({ value: source.format, label: source.title })),
        ]}
        onSelect={(value) => {
          if (value === "create") {
            onCreate();
            return;
          }
          const source = sources.find((item) => item.format === value);
          if (!source || !input.current) return;
          chosen.current = source;
          input.current.accept = source.accept;
          input.current.click();
        }}
        onClose={onClose}
      />
      <input
        ref={input}
        hidden
        type="file"
        aria-label="选择词库文件"
        onChange={(event) => {
          const file = event.target.files?.[0];
          const source = chosen.current;
          event.currentTarget.value = "";
          if (file && source) onFile(source, file);
          else onClose();
        }}
      />
    </>
  );
}
