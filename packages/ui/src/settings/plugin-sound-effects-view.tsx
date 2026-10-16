import { GroupList, MoreOptions, Row } from "../core/platform-controls";
import { musicPackToSelect, type KeySoundMode, type PluginPreferences } from "./plugin-preferences";
import {
  effectStyleOptions,
  packLabel,
  selectedPack,
  type SoundUse,
} from "./plugin-catalog-helpers";
import { PluginViewHeader } from "./plugin-view-header";
import type { PluginKind, PluginPackage } from "./plugin-types";
import { SegmentedRow } from "./segmented-row";
import { SliderRow } from "./slider-row";
import { SummaryRow } from "./summary-row";
import { SwitchRow } from "./switch-row";

const keySoundModes: readonly { value: KeySoundMode; label: string }[] = [
  { value: "keys", label: "按键音效" },
  { value: "melody", label: "按键旋律" },
];

export interface PluginSoundEffectsViewProps {
  packages: readonly PluginPackage[];
  /** A catalog has been read from a pack store, so a selection it does not list is missing rather than merely unlisted. */
  listed: boolean;
  /** The host has a pack store, so other packs can be opened from 我的插件; without one only the built-in packs are played and nothing is listed to switch to. */
  hasClient: boolean;
  preferences: PluginPreferences;
  keySound: boolean;
  music: boolean;
  typingEffects: boolean;
  effectStyles: boolean;
  effectPacks: boolean;
  /** 宿主把每种样式都画成候选卡片闪一下，越强的样式闪得越亮，不画火花（HarmonyOS）。 */
  effectFlashOnly?: boolean;
  /** 宿主按 `plugins.effect_intensity` 画效果，所以给出「效果强度」；固定强度的宿主（macOS、Windows、鸿蒙，`HostCapabilities.fixed_effect_intensity`）不给。 */
  effectIntensity?: boolean;
  /** 打字统计是否打开（`typing-statistics.json` 的 `enabled`）。成就按打字统计计数，关着时「成就音效」不能打开；不知道时（还没读到，或宿主没有打字统计）按打开处理。 */
  statisticsEnabled?: boolean;
  onChange: (preferences: PluginPreferences) => void;
  onBack: () => void;
}

/** 声音与效果: the switches, modes and volumes that belong to no one pack. Which pack is current is chosen on the pack's own detail, so here it is only named. */
export function PluginSoundEffectsView({
  packages,
  listed,
  hasClient,
  preferences,
  keySound,
  music,
  typingEffects,
  effectStyles,
  effectPacks,
  effectFlashOnly = false,
  effectIntensity = true,
  statisticsEnabled,
  onChange,
  onBack,
}: PluginSoundEffectsViewProps) {
  const { key_sound, commit_sound, melody, achievements } = preferences;
  // A selected pack replaces the style and intensity, so those two controls only describe what is drawn while no pack is selected.
  const effectPackSelected = effectPacks && preferences.effect_pack !== "";
  const selectedName = (kind: PluginKind, id: string, none: string, soundUse?: SoundUse) => {
    if (!id) return none;
    const pack = selectedPack(packages, kind, id, soundUse);
    if (pack) return packLabel(pack);
    // A host with no pack store lists nothing, so the selection is shown as it is rather than as missing.
    if (!listed) return id;
    // A sound pack installed in the other mode cannot be played the way it is selected.
    return packages.some((item) => item.kind === kind && item.id === id)
      ? `${id}（不可用）`
      : `${id}（未找到）`;
  };
  const choose = hasClient ? "在「我的插件」里打开一个包即可更换。" : "这台设备只能使用内置的包。";
  // 宿主只在开关打开并且选了音乐包时播放；还没选包时，打开开关顺手选一个随带的音乐包，而不是打开一个听不到声音的开关。一个音乐包都没有时开关不能打开。
  const musicPack = preferences.music.pack || musicPackToSelect(packages, listed);
  // 成就按打字统计计数，统计关着时打开它什么也不会响。已经打开的仍然可以关掉，不把用户锁在一个改不回来的状态里。
  const achievementsBlocked = statisticsEnabled === false && !achievements.enabled;
  // 升档音由 host-api 在连击升档时从音效包播放，不画任何样式的宿主（Linux）也会响，所以只看宿主有没有打字效果（连击计数）。
  const tierSound = typingEffects;
  return (
    <>
      <PluginViewHeader title="声音与效果" onBack={onBack} />
      {keySound && (
        <GroupList title="按键音效">
          <SwitchRow
            title="按键音"
            description="打字时按键发声。密码等安全输入框中不发声。"
            checked={key_sound.enabled}
            onChange={(enabled) =>
              onChange({ ...preferences, key_sound: { ...key_sound, enabled } })
            }
          />
          <SegmentedRow
            title="发声方式"
            description="按键音效按普通键、空格、回车和退格各自发声；按键旋律每按一键弹出旋律的下一个音，停顿 3 秒后从头开始。"
            options={keySoundModes}
            value={key_sound.mode}
            disabled={!key_sound.enabled}
            onChange={(mode) => onChange({ ...preferences, key_sound: { ...key_sound, mode } })}
          />
          <SummaryRow
            title="音效包"
            description={`按键、上屏和成就音效都取自这个音效包。${choose}`}
          >
            {selectedName("sound", key_sound.pack, key_sound.pack, "keys")}
          </SummaryRow>
          <SummaryRow title="旋律" description={choose} hidden={key_sound.mode !== "melody"}>
            {selectedName("sound", melody.pack, melody.pack, "sequence")}
          </SummaryRow>
          <SliderRow
            title="音效音量"
            description="按键音、旋律、上屏音和成就音效共用。"
            value={key_sound.volume}
            valueText={`${key_sound.volume}%`}
            onChange={(volume) => onChange({ ...preferences, key_sound: { ...key_sound, volume } })}
          />
          <SwitchRow
            title="上屏音"
            description="文字上屏时播放音效包的上屏音，与按键音各自开关。"
            checked={commit_sound.enabled}
            onChange={(enabled) => onChange({ ...preferences, commit_sound: { enabled } })}
          />
          {/* 成就音效和升档音都是默认关闭的小玩法，收进「更多音效」，主列表只留常用的几项。 */}
          <MoreOptions title="更多音效">
            <SwitchRow
              title="成就音效"
              description={
                achievementsBlocked
                  ? "上屏字数累计达到 100、1000、1 万等里程碑时播放一段短音。按打字统计计数，请先在「打字统计」页打开记录。"
                  : "上屏字数累计达到 100、1000、1 万等里程碑时播放一段短音。按打字统计计数，需要打开打字统计。"
              }
              checked={achievements.enabled}
              disabled={achievementsBlocked}
              onChange={(enabled) => onChange({ ...preferences, achievements: { enabled } })}
            />
            {tierSound && (
              <SwitchRow
                title="升档音"
                description="连击达到 10、25、50、100 时播放音效包的上屏音，每升一档音调更高，音量随音效音量。需要打开「打字效果」里的连击计数。"
                checked={preferences.combo_tier_sound}
                disabled={!preferences.combo_counter}
                onChange={(combo_tier_sound) => onChange({ ...preferences, combo_tier_sound })}
              />
            )}
          </MoreOptions>
        </GroupList>
      )}
      {typingEffects && (
        <GroupList title="打字效果">
          {effectStyles && (
            <>
              {effectPacks && (
                <SummaryRow
                  title="特效包"
                  description={
                    effectPackSelected
                      ? "特效包决定样式、强度、颜色和时长，下面的样式和强度不再生效。"
                      : choose
                  }
                >
                  {selectedName("effect", preferences.effect_pack, "不使用")}
                </SummaryRow>
              )}
              <SegmentedRow
                title="效果样式"
                description={
                  effectFlashOnly
                    ? "这台设备上各样式都只让候选栏闪一下，不迸出火花：闪光最淡，火花更亮，Power Mode 最亮，连击升档时再亮一些。"
                    : "闪光：按键时候选栏闪一下；火花：按键和上屏时迸出火花；Power Mode：火花随连击变大。"
                }
                options={effectStyleOptions}
                value={preferences.effect_style}
                disabled={effectPackSelected}
                onChange={(effect_style) => onChange({ ...preferences, effect_style })}
              />
              {effectIntensity && (
                <SliderRow
                  title="效果强度"
                  description="效果的大小和持续时间。"
                  value={preferences.effect_intensity}
                  valueText={`${preferences.effect_intensity}%`}
                  disabled={effectPackSelected || preferences.effect_style === "off"}
                  onChange={(effect_intensity) => onChange({ ...preferences, effect_intensity })}
                />
              )}
            </>
          )}
          <SwitchRow
            title="连击计数"
            description="连续打字时在候选栏显示连击数，停顿 3 秒或按退格后重新计数。自动重复的按键不计数。"
            checked={preferences.combo_counter}
            onChange={(combo_counter) => onChange({ ...preferences, combo_counter })}
          />
        </GroupList>
      )}
      {music && (
        <GroupList title="背景音乐">
          <SwitchRow
            title="背景音乐"
            description="默认关闭。只在输入法处于活动状态时播放，切换到其他输入法时暂停。"
            checked={preferences.music.enabled}
            disabled={!preferences.music.enabled && !musicPack}
            onChange={(enabled) =>
              onChange({
                ...preferences,
                music: {
                  ...preferences.music,
                  enabled,
                  pack: enabled ? musicPack : preferences.music.pack,
                },
              })
            }
          />
          <SummaryRow
            title="音乐包"
            description={
              listed && !packages.some((pack) => pack.kind === "music")
                ? "还没有导入音乐包。"
                : choose
            }
          >
            {selectedName("music", preferences.music.pack, "未选择")}
          </SummaryRow>
          <SliderRow
            title="音乐音量"
            value={preferences.music.volume}
            valueText={`${preferences.music.volume}%`}
            onChange={(volume) =>
              onChange({ ...preferences, music: { ...preferences.music, volume } })
            }
          />
        </GroupList>
      )}
    </>
  );
}
