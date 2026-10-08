import { useEffect, useRef, useState } from "react";
import { SettingsGroupNote } from "./settings-group-note";
import * as surface from "../keyboard/panel-surface-style";
import { GroupList } from "../core/platform-controls";
import { handwritingPrivacyText } from "./handwriting-platform-notice";
import * as settings from "./settings-style";
import { OpenPanelRow } from "./open-panel-row";
import { ActionRow } from "./action-row";
import { SettingsPreviewBlock } from "./settings-preview-block";
import { SelectRow } from "./select-row";
import { SliderRow } from "./slider-row";
import { useOptionalSettingsForm } from "./settings-form-context";
import type {
  HandwritingStrokeColor,
  MobileKeyboardFeedback,
} from "./mobile-keyboard-feedback-section";

/** 文件里没有值时 HarmonyOS 键盘绘制所用的默认值（`KeyboardFeedback.DEFAULTS`），用于已加载设置中缺少的字段。 */
const DEFAULT_DELAY_MS = 600;
const DEFAULT_STROKE_COLOR: HandwritingStrokeColor = "follow_skin";
const DEFAULT_STROKE_WIDTH = 3;
/** 滑块需要静止多久才保存其值。 */
const SLIDER_SAVE_DELAY_MS = 300;

const strokeColorOptions: readonly [HandwritingStrokeColor, string][] = [
  ["follow_skin", "跟随皮肤"],
  ["black", "黑色"],
  ["white", "白色"],
  ["blue", "蓝色"],
];

/** 分组与其下方的说明，两者间距与分组标题到其各行的间距一样紧凑。 */
const groupWithFooter = "flex min-w-0 flex-col gap-[var(--p-g-title-gap)]";
/** HarmonyOS 手机上分组下方 13px 的脚注，与分组标题对齐。 */
const groupFooter =
  "m-0 [padding:var(--p-g-title-pad)] text-[13px] leading-[18px] [color:var(--p-sub)]";

/**
 * 滑块移动期间其值保存在页面上，静止后保存一次。否则拖动的每一步都会单独保存一次键盘设置文件，而一次仍在进行的保存会让下一次保存变成空操作，从而丢掉手指最后停留的值。
 */
function useSettledSlider(value: number, save: (value: number) => void) {
  const [moving, setMoving] = useState<number | null>(null);
  const saveRef = useRef(save);
  saveRef.current = save;
  useEffect(() => {
    if (moving === null) return;
    const timer = setTimeout(() => {
      // 在丢弃本地值的同一个 tick 里保存，这样已保存的值（在等待保存之前就已设置）会直接接替，中间不会闪现旧值。
      if (moving !== value) saveRef.current(moving);
      setMoving(null);
    }, SLIDER_SAVE_DELAY_MS);
    return () => clearTimeout(timer);
  }, [moving, value]);
  return [moving ?? value, setMoving] as const;
}

/** HarmonyOS 手机的手写板设置：「书写」（识别延迟）和「笔迹」（墨迹颜色和粗细），隐私说明位于其下方，与 Android 的 `HandwritingPage` 的绘制一致。它们保存在键盘自己的设置文件里，所以改动会在键盘下次显示时生效。 */
function HarmonyHandwritingPadGroups({
  value,
  busy,
  onChange,
}: {
  value: MobileKeyboardFeedback;
  busy: boolean;
  onChange: (value: MobileKeyboardFeedback) => void;
}) {
  const [delay, setDelay] = useSettledSlider(
    value.handwritingDelayMs ?? DEFAULT_DELAY_MS,
    (handwritingDelayMs) => onChange({ ...value, handwritingDelayMs }),
  );
  const [width, setWidth] = useSettledSlider(
    value.handwritingStrokeWidth ?? DEFAULT_STROKE_WIDTH,
    (handwritingStrokeWidth) => onChange({ ...value, handwritingStrokeWidth }),
  );
  return (
    <>
      <GroupList title="书写">
        <SliderRow
          title="识别等待时间"
          aria-label="识别等待时间"
          min={200}
          max={1500}
          step={100}
          value={delay}
          valueText={`${delay}ms`}
          displayValue={`${delay}ms`}
          onChange={setDelay}
        />
      </GroupList>
      <div className={groupWithFooter}>
        <GroupList title="笔迹">
          <SelectRow
            title="笔迹颜色"
            aria-label="笔迹颜色"
            disabled={busy}
            value={value.handwritingStrokeColor ?? DEFAULT_STROKE_COLOR}
            onChange={(event) =>
              onChange({
                ...value,
                handwritingStrokeColor: event.target.value as HandwritingStrokeColor,
              })
            }
          >
            {strokeColorOptions.map(([color, label]) => (
              <option key={color} value={color}>
                {label}
              </option>
            ))}
          </SelectRow>
          <SliderRow
            title="笔迹粗细"
            aria-label="笔迹粗细"
            min={1}
            max={8}
            step={1}
            value={width}
            valueText={`${width}px`}
            displayValue={`${width}px`}
            onChange={setWidth}
          />
        </GroupList>
        <p className={groupFooter}>{handwritingPrivacyText("harmony")}</p>
      </div>
    </>
  );
}

/** 「手写输入」页每个平台一组：在键盘类宿主上是开启手写的方法、系统设置按钮、隐私说明和 SDK 的隐私行；在会打开自己面板的桌面宿主上是启动按钮和预览。 */
export function HandwritingSettingsSection({
  ios,
  android,
  harmony,
  macos,
  mobile,
  openSystemKeyboardSettings,
  openHandwriting,
  onOpenHandwriting,
  onOpenSdkPrivacy,
}: {
  ios: boolean;
  android: boolean;
  harmony: boolean;
  macos: boolean;
  mobile: boolean;
  openSystemKeyboardSettings?: () => void | Promise<void>;
  openHandwriting?: () => void | Promise<void>;
  onOpenHandwriting: () => void;
  /** 打开 Google ML Kit 的隐私条款；宿主无法打开外部链接时不传，这一行随之隐藏。鸿蒙用系统能力识别，从不显示它。 */
  onOpenSdkPrivacy?: () => void;
}) {
  const form = useOptionalSettingsForm();
  const padSettings = form?.mobileKeyboardFeedback;
  // HarmonyOS 手机以手写板的「书写」和「笔迹」两组开头，隐私说明移到「笔迹」下方作为脚注，启用手写的入口放在它们之后。只有上报了这些字段的键盘才会读取它们，所以其他键盘上不显示这两组。
  const harmonyPad =
    harmony && mobile && padSettings?.handwritingDelayMs !== undefined ? padSettings : undefined;
  const systemSettingsRow = (title: string, label: string) =>
    openSystemKeyboardSettings && (
      <ActionRow title={title} action={openSystemKeyboardSettings} label={label} />
    );
  const sdkPrivacyRow = onOpenSdkPrivacy && (
    <ActionRow
      title="隐私"
      description="Google ML Kit 的服务条款与数据说明"
      action={onOpenSdkPrivacy}
      label="手写 SDK 隐私说明"
    />
  );
  return ios ? (
    <GroupList title="iOS 键盘手写">
      <SettingsGroupNote>
        请在 iOS
        系统键盘设置中启用水杉键盘，并在键盘内切换到“手写”输入方案。首次使用会按需下载中文识别模型；需要开启“允许完全访问”才能下载模型，下载后可离线识别。
      </SettingsGroupNote>
      {systemSettingsRow("系统键盘设置", "打开系统键盘设置")}
      <SettingsGroupNote>{handwritingPrivacyText("ios")}</SettingsGroupNote>
      {sdkPrivacyRow}
    </GroupList>
  ) : android ? (
    <GroupList title="Android 键盘手写">
      <SettingsGroupNote>
        请在 Android 系统输入法设置中启用水杉键盘，再从键盘方案切换到“手写”。首次使用时按需下载
        Google ML Kit 中文手写模型；模型就绪后可离线识别。
      </SettingsGroupNote>
      {systemSettingsRow("系统输入法设置", "打开系统输入法设置")}
      <SettingsGroupNote>{handwritingPrivacyText("android")}</SettingsGroupNote>
      {sdkPrivacyRow}
    </GroupList>
  ) : harmony ? (
    <>
      {harmonyPad && form && (
        <HarmonyHandwritingPadGroups
          value={harmonyPad}
          busy={form.mobileKeyboardFeedbackBusy}
          onChange={(value) => void form.saveMobileKeyboardFeedback(value)}
        />
      )}
      <GroupList title="HarmonyOS 键盘手写">
        <SettingsGroupNote>
          {mobile
            ? "请在系统输入法设置中启用水杉输入法，再从键盘的方案选择器切换到“手写”。"
            : "请在系统输入法设置中启用水杉输入法；2-in-1 候选窗口不绘制键面，请先从悬浮工具栏打开屏幕键盘，再从方案选择器切换到“手写”。"}
        </SettingsGroupNote>
        {systemSettingsRow("系统输入法设置", "打开系统输入法设置")}
        {!harmonyPad && <SettingsGroupNote>{handwritingPrivacyText("harmony")}</SettingsGroupNote>}
      </GroupList>
    </>
  ) : macos ? (
    <GroupList title="macOS 手写识别板">
      <SettingsGroupNote>
        请在要输入的应用里，从输入法悬浮工具栏或输入法菜单打开手写面板；识别出的字直接输入到该应用。
      </SettingsGroupNote>
    </GroupList>
  ) : (
    <GroupList title="手写识别板">
      <OpenPanelRow
        title="打开手写识别板"
        description="使用鼠标或触控方式手写输入，自动识别候选汉字"
        action={onOpenHandwriting}
        className={`secondary ${settings.openButton}`}
      />
      <SettingsPreviewBlock aria-label="手写识别板预览">
        <div className={surface.mock}>
          <div className={surface.mockCanvas}>
            <span className={surface.mockStroke}>水</span>
          </div>
          <div className={surface.mockCandidates}>
            <span>水</span>
            <span>永</span>
            <span>木</span>
            <span>未</span>
          </div>
        </div>
      </SettingsPreviewBlock>
    </GroupList>
  );
}
