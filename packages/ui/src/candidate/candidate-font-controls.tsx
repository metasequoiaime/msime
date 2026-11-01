import { defaultCandidateEnglishFont, defaultCandidateFontFamily } from "./candidate-font-family";
import type { CandidateFontPreferences } from "./candidate-preferences";
import { candidateMainFontPatch } from "./candidate-font-presets";
import { useFontCatalog, type FontCatalogReader } from "./font-catalog";
import { FontFamilyInput } from "./font-family-input";
import { ActionButton } from "../core/action-button";
import { Row } from "../core/platform-controls";

/**
 * The candidate font rows. `candidate_fallback_fonts` has no editor here on purpose: the 候选字体 presets and, on Windows, the main font write it, every host appends the system font after it, and an ordered list of family names - most of them other platforms' faces, since the document is shared - asked users to make a typesetting decision they had no way to judge.
 *
 * 系统字体列表的读取状态和「刷新字体列表」不再单独占一行，而是跟着这里画出的第一个字体框：状态写在那一行的说明里，刷新按钮在输入框旁边。刷新是读取失败后唯一的重试办法，也是窗口开着时装了新字体后唯一能让它出现在列表里的办法，所以只要宿主能列字体就一直在。`mainFont` 和 `englishFont` 让页面把两种字体分开放（「候选窗口」页主字体在主流程，英文字体在「更多选项」里），各自带一份状态和刷新。
 */
export function CandidateFontControls({
  value,
  onChange,
  readFonts,
  windows = false,
  mainFont = true,
  englishFont = windows,
}: {
  value: CandidateFontPreferences;
  onChange: (patch: CandidateFontPreferences) => void;
  readFonts?: FontCatalogReader;
  windows?: boolean;
  /** 画「主字体」一行，缺省为画。 */
  mainFont?: boolean;
  englishFont?: boolean;
}) {
  const catalog = useFontCatalog(readFonts);
  const englishFontValue =
    value.candidate_english_font ??
    (windows
      ? defaultCandidateEnglishFont
      : (value.candidate_font_family ?? defaultCandidateFontFamily));
  const catalogStatus =
    catalog.status === "unsupported"
      ? "当前宿主未接入系统字体列表，请输入完整字体名。"
      : catalog.status === "loading"
        ? "正在读取字体列表。"
        : catalog.status === "failed"
          ? "读取字体列表失败，可重试或手动输入。"
          : catalog.status === "ready" && !catalog.fonts.length
            ? "系统字体列表为空，可手动输入。"
            : "";
  // The live region stays mounted while empty, so the status it later takes is announced.
  const status = <span role="status">{catalogStatus}</span>;
  const refresh = readFonts && (
    <ActionButton
      action={catalog.refresh}
      className="candidate-color-reset"
      disabled={catalog.status === "loading"}
      ariaLabel="刷新字体列表"
      label="刷新"
    />
  );
  const field = (
    label: string,
    fieldValue: string,
    onFieldChange: (font: string) => void,
    withRefresh: boolean,
  ) => (
    <span className="candidate-color-control">
      <FontFamilyInput
        label={label}
        value={fieldValue}
        fonts={catalog.fonts}
        enabled={!!readFonts}
        ready={catalog.status === "ready"}
        request={catalog.request}
        onChange={onFieldChange}
      />
      {withRefresh && refresh}
    </span>
  );
  const englishDescription = `优先用于候选和预编辑；缺字时使用主字体，不限英文输入模式。${windows ? "保存后自动应用。" : "未设置时跟随候选主字体。"}`;
  // 「候选窗口」页的字体行：主字体在主流程「候选字体」预设下面（字体不是预设或点了「自定义…」时才出现），英文字体在「更多选项」里；每种字体一行，输入控件在行尾，主字体排在最前，因为英文字体会回退到它。
  return (
    <>
      {mainFont && (
        <Row title="主字体" detail={status}>
          {field(
            "主字体",
            value.candidate_font_family ?? defaultCandidateFontFamily,
            (font) => onChange(candidateMainFontPatch(font, windows, value)),
            true,
          )}
        </Row>
      )}
      {/* 主字体那一行在时，状态和刷新跟着它，英文字体这一行只放输入框。 */}
      {englishFont && (
        <Row
          title="英文字体"
          description={englishDescription}
          detail={mainFont ? undefined : <>{status}</>}
        >
          {field(
            "英文字体",
            englishFontValue,
            (font) => onChange({ candidate_english_font: font }),
            !mainFont,
          )}
        </Row>
      )}
    </>
  );
}
