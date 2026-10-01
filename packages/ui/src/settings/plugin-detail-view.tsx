import type { ReactNode } from "react";
import { GroupList, Row, Switch } from "../core/platform-controls";
import * as controls from "../core/platform-controls-style";
import * as settings from "./settings-style";
import {
  MAX_COMMAND_TABLES,
  withPackSelected,
  DEFAULT_MELODY_PACK,
  DEFAULT_SOUND_PACK,
  withoutRemovedPack,
  type PluginPreferences,
} from "./plugin-preferences";
import {
  effectStyleLabel,
  kindLabels,
  missingReason,
  missingTitle,
  type MissingSelection,
} from "./plugin-catalog-helpers";
import { PluginViewHeader } from "./plugin-view-header";
import type { PluginPackage } from "./plugin-types";

/** A row whose trailing edge is a read-only value rather than a control. */
function InfoRow({ title, children }: { title: string; children: ReactNode }) {
  return (
    <Row title={title}>
      <span className={`${controls.rowDescription} text-right break-anywhere`}>{children}</span>
    </Row>
  );
}

function ActionBlock({ note, children }: { note?: ReactNode; children?: ReactNode }) {
  return (
    <div className={settings.managerBlock}>
      {note && <p className={settings.managerNote}>{note}</p>}
      {children && <div className={settings.managerActions}>{children}</div>}
    </div>
  );
}

export interface PluginDetailViewProps {
  pack: PluginPackage;
  preferences: PluginPreferences;
  keySound: boolean;
  music: boolean;
  triggers: boolean;
  /** The host draws an installed effect pack: `typingEffects && effectStyles && effectPacks`. */
  effectPacks: boolean;
  working: boolean;
  onChange: (preferences: PluginPreferences) => void;
  onCommandTable: (id: string, enabled: boolean) => void;
  onRemove: (pack: PluginPackage) => void;
  onBack: () => void;
}

/** One installed pack: what is in it, how to make it the current one of its kind, and, for a pack that is not built in, how to delete it. */
export function PluginDetailView({
  pack,
  preferences,
  keySound,
  music,
  triggers,
  effectPacks,
  working,
  onChange,
  onCommandTable,
  onRemove,
  onBack,
}: PluginDetailViewProps) {
  const select = () => onChange(withPackSelected(preferences, pack));
  return (
    <>
      <PluginViewHeader title={pack.name} onBack={onBack} />
      <GroupList title="信息">
        <InfoRow title="类型">
          {pack.kind === "sound" && pack.mode === "sequence"
            ? "音效包 · 按键旋律"
            : kindLabels[pack.kind]}
        </InfoRow>
        <InfoRow title="版本">{pack.version}</InfoRow>
        {pack.author && <InfoRow title="作者">{pack.author}</InfoRow>}
        <InfoRow title="许可证">{pack.license}</InfoRow>
        {pack.builtin && <InfoRow title="来源">内置，不能删除</InfoRow>}
        {pack.description && <p className={settings.groupNote}>{pack.description}</p>}
      </GroupList>
      <PackContent pack={pack} />
      <GroupList title="使用">
        <PackActions
          pack={pack}
          preferences={preferences}
          keySound={keySound}
          music={music}
          triggers={triggers}
          effectPacks={effectPacks}
          onSelect={select}
          onChange={onChange}
          onCommandTable={onCommandTable}
        />
      </GroupList>
      {!pack.builtin && (
        <GroupList>
          <ActionBlock note="删除后包里的文件会从本机移除；正在使用的话，设置会回到默认选择。">
            <button
              type="button"
              className="secondary"
              aria-label={`删除${pack.name}`}
              disabled={working}
              onClick={() => onRemove(pack)}
            >
              删除
            </button>
          </ActionBlock>
        </GroupList>
      )}
    </>
  );
}

/** The kind-specific content: a music pack's tracks, a command table's commands, an effect pack's style and parameters. */
function PackContent({ pack }: { pack: PluginPackage }) {
  if (pack.kind === "music") {
    const tracks = pack.tracks ?? [];
    return (
      <GroupList title={`曲目（${tracks.length}）`}>
        {tracks.length === 0 ? (
          <p className={settings.groupNote}>没有曲目。</p>
        ) : (
          tracks.map((track, index) => <Row key={`${index}/${track}`} title={track} />)
        )}
      </GroupList>
    );
  }
  if (pack.kind === "command_table") {
    const commands = pack.commands ?? [];
    return (
      <GroupList title={`指令（${commands.length}）`}>
        {commands.length === 0 ? (
          <p className={settings.groupNote}>没有指令。</p>
        ) : (
          commands.map((command) => (
            <Row key={command.trigger} title={`/${command.trigger}`} description={command.title} />
          ))
        )}
      </GroupList>
    );
  }
  if (pack.kind === "effect") {
    return (
      <GroupList title="特效">
        {pack.style && <InfoRow title="样式">{effectStyleLabel(pack.style)}</InfoRow>}
        {pack.intensity !== undefined && <InfoRow title="强度">{pack.intensity}%</InfoRow>}
        {pack.colors && pack.colors.length > 0 && (
          <InfoRow title="颜色">
            <span className="inline-flex flex-wrap items-center justify-end gap-1.5">
              {pack.colors.map((color, index) => (
                <span key={`${index}/${color}`} className="inline-flex items-center gap-1">
                  <span
                    aria-hidden="true"
                    className="inline-block size-3 rounded-full border border-edge"
                    style={{ background: color }}
                  />
                  {color}
                </span>
              ))}
            </span>
          </InfoRow>
        )}
        {pack.duration_ms != null && <InfoRow title="时长">{pack.duration_ms} 毫秒</InfoRow>}
        {pack.particles != null && <InfoRow title="粒子数">{pack.particles}</InfoRow>}
      </GroupList>
    );
  }
  return null;
}

function PackActions({
  pack,
  preferences,
  keySound,
  music,
  triggers,
  effectPacks,
  onSelect,
  onChange,
  onCommandTable,
}: {
  pack: PluginPackage;
  preferences: PluginPreferences;
  keySound: boolean;
  music: boolean;
  triggers: boolean;
  effectPacks: boolean;
  onSelect: () => void;
  onChange: (preferences: PluginPreferences) => void;
  onCommandTable: (id: string, enabled: boolean) => void;
}) {
  switch (pack.kind) {
    case "sound": {
      if (!keySound) return <ActionBlock note="这台设备不播放音效。" />;
      if (pack.mode === "sequence") {
        const current = preferences.melody.pack === pack.id;
        return (
          <ActionBlock
            note={
              preferences.key_sound.mode === "melody"
                ? "按键旋律每按一键弹出旋律的下一个音，停顿 3 秒后从头开始。"
                : "在「声音与效果」把发声方式设为按键旋律后，打字时就会弹这段旋律。"
            }
          >
            <button type="button" className="secondary" disabled={current} onClick={onSelect}>
              {current ? "使用中" : "设为按键旋律"}
            </button>
          </ActionBlock>
        );
      }
      const current = preferences.key_sound.pack === pack.id;
      return (
        <ActionBlock note="按键、上屏和成就音效都取自当前音效包；开关和音量在「声音与效果」里。">
          <button type="button" className="secondary" disabled={current} onClick={onSelect}>
            {current ? "使用中" : "设为当前音效包"}
          </button>
        </ActionBlock>
      );
    }
    case "effect": {
      if (!effectPacks) return <ActionBlock note="这台设备不绘制特效包。" />;
      const current = preferences.effect_pack === pack.id;
      return (
        <ActionBlock note="特效包决定样式、强度、颜色和时长，使用后「声音与效果」里的样式和强度不再生效。">
          {current ? (
            <button
              type="button"
              className="secondary"
              onClick={() => onChange({ ...preferences, effect_pack: "" })}
            >
              停用
            </button>
          ) : (
            <button type="button" className="secondary" onClick={onSelect}>
              使用此特效包
            </button>
          )}
        </ActionBlock>
      );
    }
    case "music": {
      if (!music) return <ActionBlock note="这台设备不播放背景音乐。" />;
      const current = preferences.music.pack === pack.id;
      return (
        <ActionBlock
          note={
            preferences.music.enabled
              ? "背景音乐只在输入法处于活动状态时播放。"
              : "在「声音与效果」打开背景音乐后播放。"
          }
        >
          <button type="button" className="secondary" disabled={current} onClick={onSelect}>
            {current ? "使用中" : "设为当前音乐包"}
          </button>
          {current && (
            <button
              type="button"
              className="secondary"
              onClick={() =>
                onChange({ ...preferences, music: { ...preferences.music, pack: "" } })
              }
            >
              不再使用
            </button>
          )}
        </ActionBlock>
      );
    }
    case "command_table": {
      if (!triggers) return <ActionBlock note="这台设备不支持 / 指令。" />;
      const position = preferences.command_tables.indexOf(pack.id);
      const full = position < 0 && preferences.command_tables.length >= MAX_COMMAND_TABLES;
      return (
        <Row
          title="启用"
          description={
            position >= 0
              ? `第 ${position + 1} 位。同一指令以靠前的表为准。`
              : full
                ? `最多启用 ${MAX_COMMAND_TABLES} 个指令表。`
                : "启用后排在已启用的指令表之后。"
          }
        >
          <Switch
            checked={position >= 0}
            disabled={full}
            onChange={(enabled) => onCommandTable(pack.id, enabled)}
          />
        </Row>
      );
    }
  }
}

/** A selection that names a pack no longer on this machine, or a sound pack in the wrong mode, with the one way out: drop it the way removing the pack would have. */
export function MissingPluginView({
  entry,
  preferences,
  onChange,
  onBack,
}: {
  entry: MissingSelection;
  preferences: PluginPreferences;
  onChange: (preferences: PluginPreferences) => void;
  onBack: () => void;
}) {
  const label = kindLabels[entry.kind];
  const action =
    entry.kind === "sound" ? "改回默认" : entry.kind === "command_table" ? "移除" : "不再使用";
  const note = entry.mismatched
    ? `设置里${entry.uses.join("、")}是「${entry.id}」，${missingReason(entry)}。可以${action}，或者在「我的插件」里打开另一个${label}。`
    : `设置里${entry.uses.join("、")}是「${entry.id}」，但这个${label}已不在本机，可能被删除或无法载入。可以重新导入它，或者${action}。`;
  // A mismatched pack is still installed and may be rightly selected in its own mode, so only the selection that cannot play it is reset.
  const dropped = (): PluginPreferences => {
    if (!entry.mismatched) return withoutRemovedPack(preferences, entry.kind, entry.id);
    return entry.uses.includes("当前音效包")
      ? { ...preferences, key_sound: { ...preferences.key_sound, pack: DEFAULT_SOUND_PACK } }
      : { ...preferences, melody: { pack: DEFAULT_MELODY_PACK } };
  };
  return (
    <>
      <PluginViewHeader title={missingTitle(entry)} onBack={onBack} />
      <GroupList>
        <ActionBlock note={note}>
          <button
            type="button"
            className="secondary"
            onClick={() => {
              onChange(dropped());
              onBack();
            }}
          >
            {action}
          </button>
        </ActionBlock>
      </GroupList>
    </>
  );
}
