import { useId } from "react";
import { InfoTip } from "../core/info-tip";
import { FluentIcon, type FluentIconName } from "../core/fluent-icons";
import { SearchGlyph } from "../core/search-glyph";
import { ActionButton } from "./action-button";
import { SettingsManagerActions } from "./settings-manager-actions";
import { MAX_COMMAND_TABLES } from "./plugin-preferences";
import {
  kindLabels,
  kindOrder,
  missingReason,
  missingTitle,
  packMarker,
} from "./plugin-catalog-helpers";
import { scriptMarker } from "./plugin-script-model";
import type { PluginListViewProps } from "./plugin-list-view";
import { CREATE_DESCRIPTION } from "./plugin-editor-model";
import type { PluginKind, PluginPackage } from "./plugin-types";
import * as style from "./plugin-library-style";

export interface PluginLibraryFilter {
  kind: PluginKind | "all";
  query: string;
}

const kindIcons: Record<PluginKind, FluentIconName> = {
  sound: "speaker_2",
  music: "speaker_2",
  effect: "sparkle",
  command_table: "braces",
  phrase_table: "document_text",
  helpcode: "keyboard",
  wordbook: "book",
  symbol_set: "text_font",
  script: "braces",
};

/** 保留插件导航的无障碍名称、描述与返回焦点标识，卡片仅改变展示。 */
function LibraryCard({
  title,
  description,
  marker,
  rowKey,
  icon,
  shortcut = false,
  onOpen,
}: {
  title: string;
  description?: string;
  marker?: string | null;
  rowKey: string;
  icon: FluentIconName;
  shortcut?: boolean;
  onOpen: () => void;
}) {
  const descriptionId = useId();
  const markerId = useId();
  return (
    <button
      type="button"
      className={shortcut ? style.shortcut : style.card}
      aria-label={title}
      aria-describedby={
        [description ? descriptionId : "", marker ? markerId : ""].filter(Boolean).join(" ") ||
        undefined
      }
      data-plugin-row={rowKey}
      onClick={onOpen}
    >
      <span className={shortcut ? style.shortcutIcon : style.packIcon} aria-hidden="true">
        <FluentIcon name={icon} size={22} />
      </span>
      <span className={shortcut ? style.shortcutText : style.cardBody}>
        <span className={shortcut ? style.shortcutTitle : style.cardTitle} title={title}>
          {title}
        </span>
        {description && (
          <span id={descriptionId} className={shortcut ? style.secondary : style.cardMeta}>
            {description}
          </span>
        )}
        {marker && (
          <span id={markerId} className={style.marker}>
            {marker}
          </span>
        )}
      </span>
      <FluentIcon name="chevron_right" className={style.chevron} />
    </button>
  );
}

/** macOS 的已安装插件库；查询与分类留在父级，详情返回不丢失定位。 */
export function PluginLibraryView({
  library: { filter, onFilter },
  hasClient,
  catalog,
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
  scripts,
}: PluginListViewProps & { library: NonNullable<PluginListViewProps["library"]> }) {
  const installed = catalog.packages.filter((pack) => pack.kind !== "script" || scripts);
  const order = scripts ? [...kindOrder, "script" as const] : kindOrder;
  const categories = order.filter(
    (kind) =>
      installed.some((pack) => pack.kind === kind) ||
      missing.some((entry) => entry.kind === kind) ||
      (kind === "command_table" && triggers),
  );
  // 目录变化后类型可能消失；立即回到全部，避免留下没有按钮可取消的筛选。
  const selectedKind = categories.includes(filter.kind as PluginKind) ? filter.kind : "all";
  const query = filter.query.trim().toLocaleLowerCase();
  const matches = (pack: PluginPackage) =>
    !query ||
    [pack.name, pack.id, pack.author, pack.description, kindLabels[pack.kind]].some((text) =>
      text?.toLocaleLowerCase().includes(query),
    );
  const groups = categories.flatMap((kind) => {
    if (selectedKind !== "all" && selectedKind !== kind) return [];
    const packs = installed.filter((pack) => pack.kind === kind && matches(pack));
    const gone = missing.filter(
      (entry) =>
        entry.kind === kind &&
        (!query || `${missingTitle(entry)} ${entry.id}`.toLocaleLowerCase().includes(query)),
    );
    return packs.length || gone.length || (kind === "command_table" && triggers && !query)
      ? [{ kind, packs, gone }]
      : [];
  });
  const issues = catalog.issues.filter((issue) => issue.kind !== "script" || scripts);
  return (
    <div className={style.library}>
      {(soundEffects || mentionsEditable) && (
        <div className={style.shortcuts}>
          {soundEffects && (
            <LibraryCard
              shortcut
              title="声音与效果"
              description={soundEffectsDescription}
              icon="speaker_2"
              rowKey="sound-effects"
              onOpen={() => onOpen("sound-effects")}
            />
          )}
          {mentionsEditable && (
            <LibraryCard
              shortcut
              title="@ 名单"
              description={mentionsDescription}
              icon="person"
              rowKey="mentions"
              onOpen={() => onOpen("mentions")}
            />
          )}
        </div>
      )}
      <div className={style.toolbar}>
        <div className={style.heading}>
          <h2 className={style.title}>
            已安装
            <span className={style.count}>
              {catalogState === "loaded" ? `${installed.length} 个插件` : ""}
            </span>
          </h2>
          {hasClient && (
            <SettingsManagerActions>
              {onCreate && (
                <ActionButton
                  action={onCreate}
                  className="primary"
                  disabled={working}
                  label="新建插件"
                />
              )}
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
            </SettingsManagerActions>
          )}
        </div>
        {notice && (
          <p role="status" className={style.note}>
            {notice}
          </p>
        )}
        <label className={style.search}>
          <SearchGlyph strokeWidth={1.5} className="size-4 shrink-0" />
          <input
            type="search"
            aria-label="搜索已安装插件"
            placeholder="搜索名称、作者或功能"
            className={style.searchInput}
            value={filter.query}
            onChange={(event) => onFilter({ ...filter, query: event.currentTarget.value })}
          />
        </label>
        <div className={style.filters} role="group" aria-label="已安装插件类型">
          <button
            type="button"
            className={style.filter}
            aria-pressed={selectedKind === "all"}
            onClick={() => onFilter({ ...filter, kind: "all" })}
          >
            全部<span>{installed.length}</span>
          </button>
          {categories.map((kind) => (
            <button
              key={kind}
              type="button"
              className={style.filter}
              aria-pressed={selectedKind === kind}
              onClick={() => onFilter({ ...filter, kind })}
            >
              {kindLabels[kind]}
              <span>{installed.filter((pack) => pack.kind === kind).length}</span>
            </button>
          ))}
        </div>
      </div>
      <div className={style.sections} aria-label="已安装的插件">
        {hasClient && catalogState === "loading" && (
          <div role="status" className={style.empty}>
            正在读取插件…
          </div>
        )}
        {hasClient && catalogState === "failed" && (
          <div className={style.empty}>插件列表没有读取成功，重新打开这个页面会再试一次。</div>
        )}
        {catalogState === "loaded" && !groups.length && (
          <div className={style.empty}>
            {query ? "没有匹配的插件，试试其他关键词。" : "没有插件"}
          </div>
        )}
        {groups.map(({ kind, packs, gone }) => (
          <section
            key={kind}
            aria-label={kind === "script" ? "脚本插件（实验）" : kindLabels[kind]}
          >
            <div className={style.sectionHeading}>
              <span className={style.sectionTitleLine}>
                <h3 className={style.sectionTitle}>
                  {kind === "script" ? "脚本插件（实验）" : kindLabels[kind]}
                </h3>
                {/* 这一类插件怎么用：收进分类名旁的「?」，不常驻页面；脚本插件的残余风险是警告，照旧画在下面。 */}
                {kind === "command_table" && triggers && (
                  <InfoTip subject={kindLabels[kind]}>
                    在「输入 → 快捷模式」打开 / 指令后，按 /
                    再输入指令字母即可使用。启用的指令表按启用顺序排列，同一指令以靠前的表为准；最多启用{" "}
                    {MAX_COMMAND_TABLES} 个。
                  </InfoTip>
                )}
                {kind === "script" && (
                  <InfoTip subject="脚本插件（实验）">
                    脚本插件会在输入法里运行第三方代码，只做你在授权时允许的事；导入后默认不运行，打开它的详情授权后才启用。
                  </InfoTip>
                )}
              </span>
              <span className={style.secondary}>{packs.length + gone.length}</span>
            </div>
            {kind === "script" && scripts?.risk && <p className={style.note}>{scripts.risk}</p>}
            {!packs.length && !gone.length && (
              <p className={style.note}>
                {hasClient
                  ? "还没有导入指令表。"
                  : "这台设备还不能导入指令表，内置的 rq、sj、xq 指令照常可用。"}
              </p>
            )}
            <div className={style.grid}>
              {packs.map((pack) => (
                <LibraryCard
                  key={pack.id}
                  title={pack.name}
                  icon={kindIcons[kind]}
                  description={[
                    pack.builtin ? "内置" : pack.version,
                    pack.kind === "sound" && pack.mode === "sequence" ? "按键旋律" : null,
                    !pack.builtin ? pack.author : null,
                  ]
                    .filter(Boolean)
                    .join(" · ")}
                  marker={
                    pack.kind === "script" && scripts
                      ? scriptMarker(pack, scripts.grants, scripts.running)
                      : packMarker(pack, preferences, kinds, modes)
                  }
                  rowKey={`pack/${pack.kind}/${pack.id}`}
                  onOpen={() => onOpenPack(pack.kind, pack.id)}
                />
              ))}
              {gone.map((entry) => (
                <LibraryCard
                  key={`missing/${entry.id}`}
                  title={missingTitle(entry)}
                  icon="info"
                  description={`${entry.uses.join("、")}，${missingReason(entry)}`}
                  rowKey={`missing/${entry.kind}/${entry.id}`}
                  onOpen={() => onOpenMissing(entry.kind, entry.id)}
                />
              ))}
            </div>
          </section>
        ))}
      </div>
      {issues.length > 0 && (
        <section aria-label="无法载入的插件">
          <h3 className={style.sectionTitle}>无法载入的插件</h3>
          <ul className={style.secondary}>
            {issues.map((issue) => (
              <li key={`${issue.kind}/${issue.folder}`}>
                {kindLabels[issue.kind]} {issue.folder || "目录"} 无法载入：{issue.reason}
              </li>
            ))}
          </ul>
        </section>
      )}
      {hasClient && (
        <details className={style.footer}>
          <summary>插件导入说明</summary>
          {onCreate && <p>{CREATE_DESCRIPTION}。</p>}
          <p>
            {scripts
              ? "数据插件只含音频、指令模板等数据；脚本插件（实验）带可执行代码，导入后要经授权才会运行。每个包须声明许可证"
              : "只含音频、指令模板等数据，不含可执行内容，每个包须声明许可证"}
          </p>
        </details>
      )}
    </div>
  );
}
