import type { InputScheme } from "../core/input-scheme";
import { SettingsGroupNote } from "./settings-group-note";

import { GroupList, Row } from "../core/platform-controls";
import {
  MAX_COMMAND_TABLES,
  MAX_PHRASE_TABLES,
  withPackSelected,
  DEFAULT_MELODY_PACK,
  DEFAULT_SOUND_PACK,
  withoutRemovedPack,
  type PluginPreferences,
} from "./plugin-preferences";
import {
  effectStyleLabel,
  kindLabels,
  melodyPlays,
  missingReason,
  missingTitle,
  packKindLabel,
  PHRASE_PREVIEW_ROWS,
  schemeTableModeNote,
  WORDBOOK_PREVIEW_WORDS,
  SYMBOL_PREVIEW_ITEMS,
  touchLocalModeEntry,
  wordbookPackBookId,
  type MissingSelection,
} from "./plugin-catalog-helpers";
import { PluginViewHeader } from "./plugin-view-header";
import { PluginInfoGroup } from "./plugin-info-group";
import type { PluginPackage, PluginSettingsPage, TouchPluginRules } from "./plugin-types";
import { SummaryRow } from "./summary-row";
import { SwitchRow } from "./switch-row";
import { ActionButton } from "./action-button";
import { PluginActionBlock } from "./plugin-action-block";
import { SettingsManagerBlock } from "./settings-manager-block";
import { SettingsManagerNote } from "./settings-manager-note";

export interface PluginDetailViewProps {
  pack: PluginPackage;
  preferences: PluginPreferences;
  keySound: boolean;
  music: boolean;
  triggers: boolean;
  /** The host draws an installed effect pack: `typingEffects && effectStyles && effectPacks`. */
  effectPacks: boolean;
  /** 宿主把每种特效样式都画成候选卡片闪一下、不画火花（HarmonyOS）。 */
  effectFlashOnly?: boolean;
  /** 快捷短语（K 模式）是否打开：`local_modes.quick_phrase`。 */
  quickPhraseMode: boolean;
  /** / 指令是否打开：`local_modes.command`。 */
  commandMode: boolean;
  /** 当前输入方案；打不开 K 和 / 模式的方案下，短语表和指令表详情会说明。缺省时不提示。 */
  scheme?: InputScheme;
  /** 宿主使用辅助码（设置里有辅助码这一组）。 */
  helpcode: boolean;
  /** 宿主的背单词书目列出单词本插件。 */
  wordbookPacks: boolean;
  /** 宿主的符号面板显示符号集插件。 */
  symbolSetPacks: boolean;
  /** 触屏设置的插件页：音效包按触屏键盘的「按键音」开关选用，模式的进入方式按触屏键盘写。桌面不传。 */
  touch?: TouchPluginRules;
  /** 在背单词里选中这本书并打开背单词；没有背单词的宿主为空。 */
  onOpenWordbook?: (book: string) => void;
  working: boolean;
  onChange: (preferences: PluginPreferences) => void;
  onCommandTable: (id: string, enabled: boolean) => void;
  onPhraseTable: (id: string, enabled: boolean) => void;
  /** 就地打开「输入 → 快捷模式」里的 / 指令；没有时改为提供跳到输入页的链接。 */
  onLocalMode?: (mode: "command" | "mention") => void;
  /** 打开设置里的另一页；宿主没有页面导航时为空，链接不显示。 */
  onOpenPage?: (page: PluginSettingsPage) => void;
  onRemove: (pack: PluginPackage) => void;
  /** 在设置页编辑器里打开这个包；只给能编辑的用户包（短语表、指令表、符号集、单词本），宿主没有编辑器时为空。 */
  onEdit?: () => void;
  /** 刚在编辑器里保存过时，顶上说明还差哪一步才能用上。 */
  notice?: string;
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
  effectFlashOnly = false,
  quickPhraseMode,
  commandMode,
  scheme,
  helpcode,
  wordbookPacks,
  symbolSetPacks,
  touch,
  onOpenWordbook,
  working,
  onChange,
  onCommandTable,
  onPhraseTable,
  onLocalMode,
  onOpenPage,
  onRemove,
  onEdit,
  notice,
  onBack,
}: PluginDetailViewProps) {
  const select = () => onChange(withPackSelected(preferences, pack));
  return (
    <>
      <PluginViewHeader title={pack.name} onBack={onBack} />
      {notice && (
        <GroupList>
          <SettingsManagerBlock>
            <SettingsManagerNote role="status">{notice}</SettingsManagerNote>
          </SettingsManagerBlock>
        </GroupList>
      )}
      <PluginInfoGroup pack={pack} kindLabel={packKindLabel(pack)}>
        {pack.builtin && (
          <SummaryRow title="来源" breakAnywhere>
            内置，不能删除
          </SummaryRow>
        )}
      </PluginInfoGroup>
      <PackContent pack={pack} />
      <GroupList title="使用">
        <PackActions
          pack={pack}
          preferences={preferences}
          keySound={keySound}
          music={music}
          triggers={triggers}
          effectPacks={effectPacks}
          effectFlashOnly={effectFlashOnly}
          quickPhraseMode={quickPhraseMode}
          commandMode={commandMode}
          scheme={scheme}
          helpcode={helpcode}
          wordbookPacks={wordbookPacks}
          symbolSetPacks={symbolSetPacks}
          touch={touch}
          onOpenWordbook={onOpenWordbook}
          onSelect={select}
          onChange={onChange}
          onCommandTable={onCommandTable}
          onPhraseTable={onPhraseTable}
          onLocalMode={onLocalMode}
          onOpenPage={onOpenPage}
        />
      </GroupList>
      {onEdit && (
        <GroupList>
          <PluginActionBlock note="不用写代码，在设置页里修改名字、介绍和内容；保存后替换本机的这一份。">
            <ActionButton
              action={onEdit}
              ariaLabel={`编辑${pack.name}`}
              disabled={working}
              label="编辑"
            />
          </PluginActionBlock>
        </GroupList>
      )}
      {!pack.builtin && (
        <GroupList>
          <PluginActionBlock note="删除后包里的文件会从本机移除；正在使用的话，设置会回到默认选择。">
            <ActionButton
              action={() => onRemove(pack)}
              ariaBusy={working}
              ariaLabel={`删除${pack.name}`}
              disabled={working}
              label="删除"
            />
          </PluginActionBlock>
        </GroupList>
      )}
    </>
  );
}

/** 各类型自己的内容：音乐包的曲目、指令表的指令、特效包的样式和参数、短语表的行数与前几行、辅助码表的条数与前几条。 */
function PackContent({ pack }: { pack: PluginPackage }) {
  if (pack.kind === "symbol_set") {
    return (
      <>
        {(pack.groups ?? []).map((group, index) => (
          <GroupList
            key={`${index}/${group.title}`}
            title={`${group.tab === "kaomoji" ? "颜文字" : "符号"} · ${group.title}（${group.items.length}）`}
          >
            <SettingsGroupNote inline className="wrap-anywhere">
              {group.items.slice(0, SYMBOL_PREVIEW_ITEMS).join(" ")}
              {group.items.length > SYMBOL_PREVIEW_ITEMS ? " …" : ""}
            </SettingsGroupNote>
          </GroupList>
        ))}
      </>
    );
  }
  if (pack.kind === "wordbook") {
    const words = (pack.first_words ?? []).slice(0, WORDBOOK_PREVIEW_WORDS);
    return (
      <GroupList title={`单词（${pack.word_count ?? words.length}）`}>
        {words.map((word) => (
          <Row key={word} title={word} />
        ))}
        {(pack.word_count ?? 0) > words.length && (
          <SettingsGroupNote inline>只显示前 {words.length} 个单词。</SettingsGroupNote>
        )}
      </GroupList>
    );
  }
  if (pack.kind === "helpcode") {
    const preview = pack.preview ?? [];
    return (
      <GroupList title={`辅助码（${pack.entries ?? preview.length} 个字）`}>
        {preview.map((entry) => (
          <Row key={entry.character} title={entry.character} detail={entry.code} />
        ))}
        {(pack.entries ?? 0) > preview.length && (
          <SettingsGroupNote inline>只显示前 {preview.length} 个字。</SettingsGroupNote>
        )}
      </GroupList>
    );
  }
  if (pack.kind === "phrase_table") {
    const phrases = pack.phrases ?? [];
    const shown = phrases.slice(0, PHRASE_PREVIEW_ROWS);
    return (
      <GroupList title={`短语（${phrases.length}）`}>
        {shown.map((phrase, index) => (
          <Row key={`${index}/${phrase.key}`} title={phrase.key} detail={phrase.text} />
        ))}
        {phrases.length > shown.length && (
          <SettingsGroupNote inline>只显示前 {PHRASE_PREVIEW_ROWS} 条。</SettingsGroupNote>
        )}
      </GroupList>
    );
  }
  if (pack.kind === "music") {
    const tracks = pack.tracks ?? [];
    return (
      <GroupList title={`曲目（${tracks.length}）`}>
        {tracks.length === 0 ? (
          <SettingsGroupNote inline>没有曲目。</SettingsGroupNote>
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
          <SettingsGroupNote inline>没有指令。</SettingsGroupNote>
        ) : (
          commands.map((command) => (
            <Row key={command.trigger} title={`/${command.trigger}`} detail={command.title} />
          ))
        )}
      </GroupList>
    );
  }
  if (pack.kind === "effect") {
    return (
      <GroupList title="特效">
        {pack.style && (
          <SummaryRow title="样式" breakAnywhere>
            {effectStyleLabel(pack.style)}
          </SummaryRow>
        )}
        {pack.intensity !== undefined && (
          <SummaryRow title="强度" breakAnywhere>
            {pack.intensity}%
          </SummaryRow>
        )}
        {pack.colors && pack.colors.length > 0 && (
          <SummaryRow title="颜色" breakAnywhere>
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
          </SummaryRow>
        )}
        {pack.duration_ms != null && (
          <SummaryRow title="时长" breakAnywhere>
            {pack.duration_ms} 毫秒
          </SummaryRow>
        )}
        {pack.particles != null && (
          <SummaryRow title="粒子数" breakAnywhere>
            {pack.particles}
          </SummaryRow>
        )}
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
  effectFlashOnly,
  quickPhraseMode,
  commandMode,
  scheme,
  helpcode,
  wordbookPacks,
  symbolSetPacks,
  touch,
  onOpenWordbook,
  onSelect,
  onChange,
  onCommandTable,
  onPhraseTable,
  onLocalMode,
  onOpenPage,
}: {
  pack: PluginPackage;
  preferences: PluginPreferences;
  keySound: boolean;
  music: boolean;
  triggers: boolean;
  effectPacks: boolean;
  effectFlashOnly: boolean;
  quickPhraseMode: boolean;
  commandMode: boolean;
  scheme?: InputScheme;
  helpcode: boolean;
  wordbookPacks: boolean;
  symbolSetPacks: boolean;
  touch?: TouchPluginRules;
  onOpenWordbook?: (book: string) => void;
  onSelect: () => void;
  onChange: (preferences: PluginPreferences) => void;
  onCommandTable: (id: string, enabled: boolean) => void;
  onPhraseTable: (id: string, enabled: boolean) => void;
  onLocalMode?: (mode: "command" | "mention") => void;
  onOpenPage?: (page: PluginSettingsPage) => void;
}) {
  switch (pack.kind) {
    case "sound": {
      if (touch)
        return (
          <TouchSoundActions
            pack={pack}
            preferences={preferences}
            touch={touch}
            onChange={onChange}
            onOpenPage={onOpenPage}
          />
        );
      if (!keySound) return <PluginActionBlock note="这台设备不播放音效。" />;
      // 选用即使用：按钮在包已选中但开关没开（之后在「声音与效果」关掉了按键音或换了发声方式）时仍可点，点了会把开关重新打开。
      if (pack.mode === "sequence") {
        const selected = preferences.melody.pack === pack.id;
        const playing = melodyPlays(preferences);
        return (
          <PluginActionBlock
            note={
              playing
                ? "按键旋律每按一键弹出旋律的下一个音，停顿 3 秒后从头开始。"
                : "按键旋律现在没有打开。设为按键旋律会同时打开按键音，并把发声方式改为按键旋律。"
            }
          >
            <ActionButton
              action={onSelect}
              disabled={selected && playing}
              label={selected ? (playing ? "使用中" : "打开按键旋律") : "设为按键旋律"}
            />
          </PluginActionBlock>
        );
      }
      const selected = preferences.key_sound.pack === pack.id;
      const keysEnabled = preferences.key_sound.enabled;
      const playing = keysEnabled && preferences.key_sound.mode === "keys";
      const resume = keysEnabled ? "改用按键音效" : "打开按键音";
      return (
        <PluginActionBlock
          note={
            playing
              ? "按键、上屏和成就音效都取自当前音效包；开关和音量在「声音与效果」里。"
              : keysEnabled
                ? "发声方式现在是按键旋律。设为当前音效包会改回按键音效；上屏和成就音效也取自当前音效包，开关在「声音与效果」里。"
                : "按键音现在没有打开。设为当前音效包会同时打开按键音；上屏和成就音效也取自当前音效包，开关在「声音与效果」里。"
          }
        >
          <ActionButton
            action={onSelect}
            disabled={selected && playing}
            label={selected ? (playing ? "使用中" : resume) : "设为当前音效包"}
          />
        </PluginActionBlock>
      );
    }
    case "effect": {
      if (!effectPacks) return <PluginActionBlock note="这台设备不绘制特效包。" />;
      const current = preferences.effect_pack === pack.id;
      return (
        <PluginActionBlock
          note={
            effectFlashOnly
              ? "特效包决定样式、强度、颜色和时长，使用后「声音与效果」里的样式和强度不再生效。这台设备只让候选栏闪光，不绘制火花：火花和 Power Mode 闪得更亮，粒子数不起作用。"
              : "特效包决定样式、强度、颜色和时长，使用后「声音与效果」里的样式和强度不再生效。"
          }
        >
          {current ? (
            <ActionButton
              action={() => onChange({ ...preferences, effect_pack: "" })}
              label="停用"
            />
          ) : (
            <ActionButton action={onSelect} label="使用此特效包" />
          )}
        </PluginActionBlock>
      );
    }
    case "music": {
      if (!music) return <PluginActionBlock note="这台设备不播放背景音乐。" />;
      const selected = preferences.music.pack === pack.id;
      const playing = preferences.music.enabled;
      return (
        <PluginActionBlock
          note={
            playing
              ? "背景音乐只在输入法处于活动状态时播放。"
              : "背景音乐现在没有打开。设为当前音乐包会同时打开它，只在输入法处于活动状态时播放。"
          }
        >
          <ActionButton
            action={onSelect}
            disabled={selected && playing}
            label={selected ? (playing ? "使用中" : "打开背景音乐") : "设为当前音乐包"}
          />
          {selected && (
            <ActionButton
              action={() => onChange({ ...preferences, music: { ...preferences.music, pack: "" } })}
              label="不再使用"
            />
          )}
        </PluginActionBlock>
      );
    }
    case "command_table": {
      if (!triggers) return <PluginActionBlock note="这台设备不支持 / 指令。" />;
      const position = preferences.command_tables.indexOf(pack.id);
      const full = position < 0 && preferences.command_tables.length >= MAX_COMMAND_TABLES;
      const schemeNote = schemeTableModeNote(scheme, "指令（/ 模式）");
      return (
        <>
          <SwitchRow
            title="启用"
            description="启用后排在已启用的指令表之后；同一指令以靠前的表为准。"
            detail={
              position >= 0
                ? `第 ${position + 1} 位`
                : full
                  ? `最多启用 ${MAX_COMMAND_TABLES} 个指令表`
                  : undefined
            }
            checked={position >= 0}
            disabled={full}
            onChange={(enabled) => onCommandTable(pack.id, enabled)}
          />
          {!commandMode && (
            <PluginActionBlock
              note={
                touch
                  ? `/ 指令（「输入 → 快捷模式」里的开关）已关闭，启用的指令表不会生效。打开后，${touchLocalModeEntry("指令")}再输入指令字母即可使用。`
                  : "/ 指令（「输入 → 快捷模式」里的开关）已关闭，启用的指令表不会生效。打开后，在中文标点下没有输入时按 / 再输入指令字母即可使用。"
              }
            >
              {onLocalMode ? (
                <ActionButton action={() => onLocalMode("command")} label="打开 / 指令" />
              ) : (
                onOpenPage && (
                  <ActionButton action={() => onOpenPage("input")} label="前往输入设置" />
                )
              )}
            </PluginActionBlock>
          )}
          {schemeNote && <PluginActionBlock note={schemeNote} />}
        </>
      );
    }
    case "symbol_set":
      return (
        <PluginActionBlock
          note={
            symbolSetPacks
              ? "装上即在符号面板里显示：符号组以插件名为分类排在内置符号之后，颜文字组排在颜文字的 All 之后。卸载后不再显示。"
              : "这台设备的符号面板不显示插件符号集。"
          }
        />
      );
    case "wordbook": {
      if (!wordbookPacks || !onOpenWordbook)
        return <PluginActionBlock note="这台设备的背单词不列出单词本插件。" />;
      return (
        <PluginActionBlock note="这本书出现在背单词的词书里。卸载插件后复习进度仍会保留，重新安装后可以接着复习。">
          <ActionButton
            action={() => onOpenWordbook(wordbookPackBookId(pack.id))}
            label="去背单词"
          />
        </PluginActionBlock>
      );
    }
    case "helpcode": {
      if (!helpcode) return <PluginActionBlock note="这台设备不使用辅助码。" />;
      const toggle = (key: "helpcode_pack_quanpin" | "helpcode_pack_shuangpin", on: boolean) =>
        onChange({ ...preferences, [key]: on ? pack.id : "" });
      return (
        <>
          <SwitchRow
            title="用于全拼"
            description="替换全拼的辅助码方案；关闭后回到原来的方案。"
            checked={preferences.helpcode_pack_quanpin === pack.id}
            onChange={(on) => toggle("helpcode_pack_quanpin", on)}
          />
          <SwitchRow
            title="用于双拼"
            description="替换双拼的辅助码方案；关闭后回到原来的方案。"
            checked={preferences.helpcode_pack_shuangpin === pack.id}
            onChange={(on) => toggle("helpcode_pack_shuangpin", on)}
          />
          {onOpenPage && (
            <PluginActionBlock note="辅助码的开关和显示方式在「输入 → 辅助码」里。">
              <ActionButton action={() => onOpenPage("input")} label="前往辅助码设置" />
            </PluginActionBlock>
          )}
        </>
      );
    }
    case "phrase_table": {
      if (!triggers) return <PluginActionBlock note="这台设备不支持快捷短语插件。" />;
      const position = preferences.phrase_tables.indexOf(pack.id);
      const full = position < 0 && preferences.phrase_tables.length >= MAX_PHRASE_TABLES;
      const schemeNote = schemeTableModeNote(scheme, "快捷短语（K 模式）");
      return (
        <>
          <SwitchRow
            title="启用"
            description="启用后排在已启用的短语表之后；同一编码下靠前的表先列出。"
            detail={
              position >= 0
                ? `第 ${position + 1} 位`
                : full
                  ? `最多启用 ${MAX_PHRASE_TABLES} 个短语表`
                  : undefined
            }
            checked={position >= 0}
            disabled={full}
            onChange={(enabled) => onPhraseTable(pack.id, enabled)}
          />
          {!quickPhraseMode && (
            <PluginActionBlock
              note={
                touch
                  ? `快捷短语（K 模式）已关闭。在「输入 → 快捷模式」打开后，${touchLocalModeEntry("快捷短语")}再输入编码即可用到短语表。`
                  : "快捷短语（K 模式）已关闭。在「输入 → 快捷模式」打开后，按 Shift+K 再输入编码即可用到短语表。"
              }
            >
              {onOpenPage && (
                <ActionButton action={() => onOpenPage("input")} label="前往输入设置" />
              )}
            </PluginActionBlock>
          )}
          {schemeNote && <PluginActionBlock note={schemeNote} />}
        </>
      );
    }
  }
}

/**
 * 触屏键盘的音效包：只播按键音，不播旋律、上屏音和成就音（宿主的 `KeySoundPolicy.touchSettings`）。包写在共享偏好 `plugins.key_sound.pack`，「按键音」开关是宿主本机的按键反馈设置，在「屏幕键盘 → 按键反馈」里，和那里的「音效」选择是同一个值。和桌面一样，设为按键音效时按键音若关着就一起打开。
 */
function TouchSoundActions({
  pack,
  preferences,
  touch,
  onChange,
  onOpenPage,
}: {
  pack: PluginPackage;
  preferences: PluginPreferences;
  touch: TouchPluginRules;
  onChange: (preferences: PluginPreferences) => void;
  onOpenPage?: (page: PluginSettingsPage) => void;
}) {
  if (pack.mode === "sequence") {
    return <PluginActionBlock note="这台设备的触屏键盘不播放按键旋律。" />;
  }
  const selected = preferences.key_sound.pack === pack.id;
  // 还没读到开关时按打开处理，与列表的标记一致。
  const soundOn = touch.keySoundEnabled !== false;
  const enable = touch.onEnableKeySound;
  const select = () => {
    onChange({ ...preferences, key_sound: { ...preferences.key_sound, pack: pack.id } });
    if (!soundOn) enable?.();
  };
  const note = soundOn
    ? "按键时播放这个音效包里的按键声，手机和平板不播放上屏和成就音效。开关和音量跟随「键盘 → 按键反馈」里的「按键音」。"
    : enable
      ? "按键音现在没有打开。设为按键音效会同时打开「键盘 → 按键反馈」里的「按键音」；手机和平板不播放上屏和成就音效。"
      : "按键音现在没有打开，在「键盘 → 按键反馈」里打开「按键音」后才会出声；手机和平板不播放上屏和成就音效。";
  return (
    <PluginActionBlock note={note}>
      {selected && !soundOn && enable ? (
        <ActionButton action={enable} label="打开按键音" />
      ) : (
        <ActionButton
          action={select}
          disabled={selected}
          label={selected ? (soundOn ? "使用中" : "已选") : "设为按键音效"}
        />
      )}
      {!soundOn && !enable && onOpenPage && (
        <ActionButton action={() => onOpenPage("screen-keyboard")} label="前往按键反馈" />
      )}
    </PluginActionBlock>
  );
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
    entry.kind === "sound"
      ? "改回默认"
      : entry.kind === "command_table" || entry.kind === "phrase_table"
        ? "移除"
        : entry.kind === "helpcode"
          ? "改回原来的方案"
          : "不再使用";
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
        <PluginActionBlock note={note}>
          <ActionButton
            action={() => {
              onChange(dropped());
              onBack();
            }}
            label={action}
          />
        </PluginActionBlock>
      </GroupList>
    </>
  );
}
