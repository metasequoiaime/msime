import { SettingsGroupNote } from "./settings-group-note";
import { SettingsManagerNote } from "./settings-manager-note";
import { SettingsManagerActions } from "./settings-manager-actions";
import { GroupList, Row } from "../core/platform-controls";
import * as settings from "./settings-style";
import { SettingsEmptyMessage } from "./settings-empty-message";
import { SettingsManagerBlock } from "./settings-manager-block";
import { MAX_COMMAND_TABLES, type PluginPreferences } from "./plugin-preferences";
import {
  kindLabels,
  kindOrder,
  missingReason,
  missingTitle,
  packKindLabel,
  packMarker,
  touchLocalModeEntry,
  type MissingSelection,
  type PluginTableModes,
} from "./plugin-catalog-helpers";
import type {
  PluginCatalogResult,
  PluginKind,
  PluginPackage,
  ScriptGrantsResult,
  TouchPluginRules,
} from "./plugin-types";
import { ActionButton } from "./action-button";
import { scriptMarker } from "./plugin-script-model";
import { PluginNavRow } from "./plugin-nav-row";
import { PluginLibraryView, type PluginLibraryFilter } from "./plugin-library-view";
import { CREATE_DESCRIPTION } from "./plugin-editor-model";

/** 「脚本插件（实验）」分组要的东西：宿主运行脚本插件时才有。 */
export type PluginListScripts = {
  packages: readonly PluginPackage[];
  /** 还没读到时为 null，行尾按未授权标记。 */
  grants: ScriptGrantsResult | null;
  /** 宿主会运行的插件 id，按执行顺序。 */
  running: readonly string[];
  /** 同一用户下的程序能伪造授权的残余风险说明（`scriptResidualRisk`），不认识的平台为 null。 */
  risk: string | null;
};

export interface PluginListViewProps {
  /** macOS 插件库的临时筛选由父级保留，返回详情时仍能恢复焦点。 */
  library?: { filter: PluginLibraryFilter; onFilter: (filter: PluginLibraryFilter) => void };
  /** Absent on a host with no pack store: nothing is listed or imported, but the 声音与效果 entry still opens. */
  hasClient: boolean;
  catalog: PluginCatalogResult;
  /** 触屏插件页不列出的已安装包（这台设备用不上的类型和按键旋律），列表末尾只说有几个。 */
  unlisted?: readonly PluginPackage[];
  /** 触屏设置的插件页（`HostCapabilities.touch_plugin_packs`）：按键音效包按触屏「按键音」开关标记，指令表的说明按触屏键盘的入口写，导入只收 .zip。 */
  touch?: TouchPluginRules;
  /** Where the first read of the catalog stands. Until it has loaded, an empty catalog says nothing about what is installed, so no empty note is shown. */
  catalogState: "loading" | "loaded" | "failed";
  missing: readonly MissingSelection[];
  /** The pack kinds the host acts on; a pack of another kind is listed but never marked as in use. */
  kinds: ReadonlySet<PluginKind>;
  preferences: PluginPreferences;
  /** K 模式、/ 指令是否打开，以及当前方案能否打开它们（`schemeOpens`）：方案打不开时已启用的短语表、指令表先标「当前方案打不开」，开关关着时标未开，否则标排位。 */
  modes: PluginTableModes;
  triggers: boolean;
  /** Whether the 声音与效果 entry has anything to open. */
  soundEffects: boolean;
  soundEffectsDescription: string;
  mentionsEditable: boolean;
  mentionsDescription: string;
  working: boolean;
  notice: string;
  onImport: (source: "folder" | "archive") => void;
  /** 打开设置页插件编辑器新建一个包；宿主没有编辑器时为空，不显示「新建插件」。 */
  onCreate?: () => void;
  onOpen: (target: "sound-effects" | "mentions") => void;
  onOpenPack: (kind: PluginKind, id: string) => void;
  onOpenMissing: (kind: PluginKind, id: string) => void;
  /** 宿主运行脚本插件时的「脚本插件（实验）」分组；不运行时为 null，脚本包和它们的问题行都不列出。 */
  scripts?: PluginListScripts | null;
}

/** 我的插件：macOS 使用可筛选的卡片库，其他宿主保留按类型分组的导航列表。 */
export function PluginListView(props: PluginListViewProps) {
  if (props.library) return <PluginLibraryView {...props} library={props.library} />;
  return <GroupedPluginListView {...props} />;
}

function GroupedPluginListView({
  hasClient,
  catalog,
  unlisted = [],
  touch,
  catalogState,
  missing,
  kinds,
  preferences,
  modes,
  triggers,
  soundEffects,
  soundEffectsDescription,
  mentionsEditable,
  mentionsDescription,
  working,
  notice,
  onImport,
  onCreate,
  onOpen,
  onOpenPack,
  onOpenMissing,
  scripts = null,
}: PluginListViewProps) {
  const groups = kindOrder.flatMap((kind) => {
    const packs = catalog.packages.filter((pack) => pack.kind === kind);
    const gone = missing.filter((entry) => entry.kind === kind);
    // The command-table group carries the notes about the / mode, so it stays while the host routes it even when empty.
    const shown = packs.length > 0 || gone.length > 0 || (kind === "command_table" && triggers);
    return shown ? [{ kind, packs, gone }] : [];
  });
  const scriptPacks = scripts?.packages ?? [];
  // 宿主不运行脚本插件时，脚本文件夹的问题行也不列出：那里根本用不上它们。
  const issues = scripts
    ? catalog.issues
    : catalog.issues.filter((issue) => issue.kind !== "script");
  // 不列出的包按类型说一句，例如「特效包、音效包·旋律」。
  const unlistedKinds = [...new Set(unlisted.map((pack) => packKindLabel(pack)))].join("、");
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
          <SettingsEmptyMessage role="status">正在读取插件…</SettingsEmptyMessage>
        )}
        {hasClient && catalogState === "failed" && (
          <SettingsEmptyMessage>
            插件列表没有读取成功，重新打开这个页面会再试一次。
          </SettingsEmptyMessage>
        )}
        {hasClient &&
          catalogState === "loaded" &&
          groups.length === 0 &&
          scriptPacks.length === 0 && <SettingsEmptyMessage>没有插件</SettingsEmptyMessage>}
        {groups.map(({ kind, packs, gone }) => (
          <GroupList key={kind} title={kindLabels[kind]}>
            {kind === "command_table" && triggers && (
              <SettingsGroupNote>
                {touch
                  ? `在「输入 → 快捷模式」打开 / 指令后，${touchLocalModeEntry("指令")}再输入指令字母即可使用。`
                  : "在「输入 → 快捷模式」打开 / 指令后，按 / 再输入指令字母即可使用。"}
                启用的指令表按启用顺序排列，同一指令以靠前的表为准；最多启用 {MAX_COMMAND_TABLES}{" "}
                个。
              </SettingsGroupNote>
            )}
            {kind === "command_table" &&
              triggers &&
              packs.length === 0 &&
              gone.length === 0 &&
              (!hasClient || catalogState === "loaded") && (
                <SettingsGroupNote inline>
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
                marker={packMarker(pack, preferences, kinds, modes, touch)}
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
        {scripts && scriptPacks.length > 0 && (
          <GroupList title="脚本插件（实验）">
            <SettingsGroupNote>
              脚本插件会在输入法里运行第三方代码，只做你在授权时允许的事；导入后默认不运行，打开它的详情授权后才启用。
            </SettingsGroupNote>
            {scripts.risk && <SettingsGroupNote inline>{scripts.risk}</SettingsGroupNote>}
            {scriptPacks.map((pack) => (
              <PluginNavRow
                key={pack.id}
                title={pack.name}
                description={
                  [pack.version, pack.author ? `作者 ${pack.author}` : null]
                    .filter(Boolean)
                    .join(" · ") || undefined
                }
                marker={scriptMarker(pack, scripts.grants, scripts.running)}
                rowKey={`pack/script/${pack.id}`}
                onOpen={() => onOpenPack("script", pack.id)}
              />
            ))}
          </GroupList>
        )}
        {unlisted.length > 0 && (
          <SettingsEmptyMessage>
            另有 {unlisted.length} 个已安装的插件（{unlistedKinds}）在这台设备上用不到，没有列出。
          </SettingsEmptyMessage>
        )}
      </div>
      {hasClient && onCreate && (
        <GroupList title="制作插件">
          <Row title="自己制作" description={CREATE_DESCRIPTION}>
            <SettingsManagerActions as="span" className="shrink-0">
              <ActionButton action={onCreate} disabled={working} label="新建插件" />
            </SettingsManagerActions>
          </Row>
        </GroupList>
      )}
      {hasClient && (
        <GroupList title="导入插件">
          <Row
            title="从文件导入"
            description={
              scripts
                ? "数据插件只含音频、指令模板等数据；脚本插件（实验）带可执行代码，导入后要经授权才会运行。每个包须声明许可证"
                : touch
                  ? "选一个插件的 .zip 压缩包。只含音频、指令模板等数据，不含可执行内容，每个包须声明许可证"
                  : "只含音频、指令模板等数据，不含可执行内容，每个包须声明许可证"
            }
          >
            <SettingsManagerActions as="span" className="shrink-0">
              {/* 手机和平板的系统文件选择器不能选文件夹，触屏插件页只收 .zip。 */}
              {!touch && (
                <ActionButton
                  action={() => onImport("folder")}
                  disabled={working}
                  label="导入文件夹"
                />
              )}
              <ActionButton
                action={() => onImport("archive")}
                disabled={working}
                label="导入 .zip"
              />
            </SettingsManagerActions>
          </Row>
          {notice && <SettingsGroupNote role="status">{notice}</SettingsGroupNote>}
        </GroupList>
      )}
      {issues.length > 0 && (
        <GroupList title="无法载入的插件">
          <SettingsManagerBlock role="list" aria-label="无法载入的插件">
            {issues.map((issue) => (
              <SettingsManagerNote role="listitem" key={`${issue.kind}/${issue.folder}`}>
                {kindLabels[issue.kind]} {issue.folder || "目录"} 无法载入：{issue.reason}
              </SettingsManagerNote>
            ))}
          </SettingsManagerBlock>
        </GroupList>
      )}
    </>
  );
}
