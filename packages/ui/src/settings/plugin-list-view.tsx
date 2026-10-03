import { SettingsGroupNote } from "./settings-group-note";
import { SettingsManagerNote } from "./settings-manager-note";
import { useId } from "react";
import { GroupList } from "../core/platform-controls";
import * as controls from "../core/platform-controls-style";
import * as settings from "./settings-style";
import { MAX_COMMAND_TABLES, type PluginPreferences } from "./plugin-preferences";
import {
  kindLabels,
  kindOrder,
  missingReason,
  missingTitle,
  packMarker,
  type MissingSelection,
} from "./plugin-catalog-helpers";
import type { PluginCatalogResult, PluginKind } from "./plugin-types";
import { ActionButton } from "./action-button";

/** A row that opens a view, in the markup `SubPageEntries` gives its rows: named by its title, described by its description and its trailing marker. `rowKey` is what focus returns to when the view is closed. */
function PluginNavRow({
  title,
  description,
  marker,
  rowKey,
  onOpen,
}: {
  title: string;
  description?: string;
  marker?: string | null;
  rowKey: string;
  onOpen: () => void;
}) {
  const descriptionId = useId();
  const markerId = useId();
  const describedBy = [description ? descriptionId : null, marker ? markerId : null]
    .filter(Boolean)
    .join(" ");
  return (
    <button
      type="button"
      className={`${controls.row} w-full cursor-pointer border-0 text-left hover:bg-[var(--p-hover)]`}
      aria-label={title}
      aria-describedby={describedBy || undefined}
      data-plugin-row={rowKey}
      onClick={onOpen}
    >
      <span className={controls.rowText}>
        <span className={controls.rowTitle}>{title}</span>
        {description && (
          <span id={descriptionId} className={controls.rowDescription}>
            {description}
          </span>
        )}
      </span>
      {marker && (
        <span id={markerId} className={`${controls.rowDescription} shrink-0`}>
          {marker}
        </span>
      )}
      <span className={controls.rowDescription} aria-hidden="true">
        ›
      </span>
    </button>
  );
}

export interface PluginListViewProps {
  /** Absent on a host with no pack store: nothing is listed or imported, but the 声音与效果 entry still opens. */
  hasClient: boolean;
  catalog: PluginCatalogResult;
  /** Where the first read of the catalog stands. Until it has loaded, an empty catalog says nothing about what is installed, so no empty note is shown. */
  catalogState: "loading" | "loaded" | "failed";
  missing: readonly MissingSelection[];
  /** The pack kinds the host acts on; a pack of another kind is listed but never marked as in use. */
  kinds: ReadonlySet<PluginKind>;
  preferences: PluginPreferences;
  triggers: boolean;
  /** Whether the 声音与效果 entry has anything to open. */
  soundEffects: boolean;
  soundEffectsDescription: string;
  mentionsEditable: boolean;
  mentionsDescription: string;
  working: boolean;
  notice: string;
  onImport: (source: "folder" | "archive") => void;
  onOpen: (target: "sound-effects" | "mentions") => void;
  onOpenPack: (kind: PluginKind, id: string) => void;
  onOpenMissing: (kind: PluginKind, id: string) => void;
}

/** 我的插件：先是不属于任何一个插件包的设置入口，再是按类型分组、各自打开详情的已安装插件，导入这一行放在最后，因为导入是少见的操作。 */
export function PluginListView({
  hasClient,
  catalog,
  catalogState,
  missing,
  kinds,
  preferences,
  triggers,
  soundEffects,
  soundEffectsDescription,
  mentionsEditable,
  mentionsDescription,
  working,
  notice,
  onImport,
  onOpen,
  onOpenPack,
  onOpenMissing,
}: PluginListViewProps) {
  const groups = kindOrder.flatMap((kind) => {
    const packs = catalog.packages.filter((pack) => pack.kind === kind);
    const gone = missing.filter((entry) => entry.kind === kind);
    // The command-table group carries the notes about the / mode, so it stays while the host routes it even when empty.
    const shown = packs.length > 0 || gone.length > 0 || (kind === "command_table" && triggers);
    return shown ? [{ kind, packs, gone }] : [];
  });
  return (
    <>
      {(soundEffects || mentionsEditable) && (
        <GroupList>
          {soundEffects && (
            <PluginNavRow
              title="声音与效果"
              description={soundEffectsDescription}
              rowKey="sound-effects"
              onOpen={() => onOpen("sound-effects")}
            />
          )}
          {mentionsEditable && (
            <PluginNavRow
              title="@ 名单"
              description={mentionsDescription}
              rowKey="mentions"
              onOpen={() => onOpen("mentions")}
            />
          )}
        </GroupList>
      )}
      <div className={settings.subViewStack} aria-label="已安装的插件">
        {hasClient && catalogState === "loading" && (
          <p className={settings.clipboardEmpty} role="status">
            正在读取插件…
          </p>
        )}
        {hasClient && catalogState === "failed" && (
          <p className={settings.clipboardEmpty}>
            插件列表没有读取成功，重新打开这个页面会再试一次。
          </p>
        )}
        {hasClient && catalogState === "loaded" && groups.length === 0 && (
          <p className={settings.clipboardEmpty}>没有插件</p>
        )}
        {groups.map(({ kind, packs, gone }) => (
          <GroupList key={kind} title={kindLabels[kind]}>
            {kind === "command_table" && triggers && (
              <SettingsGroupNote>
                在「输入 → 快捷模式」打开 / 指令后，按 /
                再输入指令字母即可使用。启用的指令表按启用顺序排列，同一指令以靠前的表为准；最多启用{" "}
                {MAX_COMMAND_TABLES} 个。
              </SettingsGroupNote>
            )}
            {kind === "command_table" &&
              triggers &&
              packs.length === 0 &&
              gone.length === 0 &&
              (!hasClient || catalogState === "loaded") && (
                <SettingsGroupNote>
                  {hasClient
                    ? "还没有导入指令表。"
                    : "这台设备还不能导入指令表，内置的 rq、sj、xq 指令照常可用。"}
                </SettingsGroupNote>
              )}
            {packs.map((pack) => (
              <PluginNavRow
                key={pack.id}
                title={pack.name}
                // 内置插件的版本和作者都一样，所以这一行只留能把它们区分开的内容；详情页仍然显示全部信息。
                description={
                  [
                    pack.builtin ? null : pack.version,
                    pack.kind === "sound" && pack.mode === "sequence" ? "按键旋律" : null,
                    !pack.builtin && pack.author ? `作者 ${pack.author}` : null,
                  ]
                    .filter(Boolean)
                    .join(" · ") || undefined
                }
                marker={packMarker(pack, preferences, kinds)}
                rowKey={`pack/${pack.kind}/${pack.id}`}
                onOpen={() => onOpenPack(pack.kind, pack.id)}
              />
            ))}
            {gone.map((entry) => (
              <PluginNavRow
                key={`missing/${entry.id}`}
                title={missingTitle(entry)}
                description={`${entry.uses.join("、")}，${missingReason(entry)}`}
                rowKey={`missing/${entry.kind}/${entry.id}`}
                onOpen={() => onOpenMissing(entry.kind, entry.id)}
              />
            ))}
          </GroupList>
        ))}
      </div>
      {hasClient && (
        <GroupList title="导入插件">
          <div className={controls.row}>
            <span className={controls.rowText}>
              <span className={controls.rowTitle}>从文件导入</span>
              <span className={controls.rowDescription}>
                只含音频、指令模板等数据，不含可执行内容，每个包须声明许可证
              </span>
            </span>
            <span className={`${settings.managerActions} shrink-0`}>
              <ActionButton
                action={() => onImport("folder")}
                disabled={working}
                label="导入文件夹"
              />
              <ActionButton
                action={() => onImport("archive")}
                disabled={working}
                label="导入 .zip"
              />
            </span>
          </div>
          {notice && <SettingsGroupNote role="status">{notice}</SettingsGroupNote>}
        </GroupList>
      )}
      {catalog.issues.length > 0 && (
        <GroupList title="无法载入的插件">
          <div className={settings.managerBlock} role="list" aria-label="无法载入的插件">
            {catalog.issues.map((issue) => (
              <SettingsManagerNote role="listitem" key={`${issue.kind}/${issue.folder}`}>
                {kindLabels[issue.kind]} {issue.folder || "目录"} 无法载入：{issue.reason}
              </SettingsManagerNote>
            ))}
          </div>
        </GroupList>
      )}
    </>
  );
}
