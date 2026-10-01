import {
  defaultCandidateEnglishFont,
  defaultCandidateFontFamily,
  type CandidateFontPreferences,
} from "./candidate-font-family";
import { useFontCatalog, type FontCatalogReader } from "./font-catalog";
import { FontFamilyInput } from "./font-family-input";
import { Row } from "../core/platform-controls";

/**
 * The candidate font rows. `candidate_fallback_fonts` has no editor here on purpose: the 候选字体 presets write it, every host appends the system font after it, and an ordered list of family names - most of them other platforms' faces, since the document is shared - asked users to make a typesetting decision they had no way to judge.
 */
export function CandidateFontControls({
  value,
  onChange,
  readFonts,
  windows = false,
  englishFont = windows,
  mobile = false,
}: {
  value: CandidateFontPreferences;
  onChange: (patch: CandidateFontPreferences) => void;
  readFonts?: FontCatalogReader;
  windows?: boolean;
  englishFont?: boolean;
  mobile?: boolean;
}) {
  const catalog = useFontCatalog(readFonts);
  const englishFontValue =
    value.candidate_english_font ??
    (windows
      ? defaultCandidateEnglishFont
      : (value.candidate_font_family ?? defaultCandidateFontFamily));
  const surfaceName = mobile ? "候选栏" : "候选窗";
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
  // Rows of the page's 字体 group: each font is a row with its input on the trailing edge.
  return (
    <>
      {englishFont && (
        <Row
          title={`${surfaceName}英文字体`}
          description={`优先用于候选和预编辑；缺字时使用主字体，不限英文输入模式。${windows ? "保存后自动应用。" : "未设置时跟随候选主字体。"}`}
        >
          <FontFamilyInput
            label={`${surfaceName}英文字体`}
            value={englishFontValue}
            fonts={catalog.fonts}
            enabled={!!readFonts}
            ready={catalog.status === "ready"}
            request={catalog.request}
            onChange={(font) => onChange({ candidate_english_font: font })}
          />
        </Row>
      )}
      {!windows && (
        <Row title={`${surfaceName}主字体`}>
          <FontFamilyInput
            label={`${surfaceName}主字体`}
            value={value.candidate_font_family ?? defaultCandidateFontFamily}
            fonts={catalog.fonts}
            enabled={!!readFonts}
            ready={catalog.status === "ready"}
            request={catalog.request}
            onChange={(font) => onChange({ candidate_font_family: font })}
          />
        </Row>
      )}
      {/* The live region stays mounted while empty, so the status it later takes is announced. */}
      <Row title="系统字体列表" description={<span role="status">{catalogStatus}</span>}>
        {readFonts && (
          <button
            type="button"
            className="secondary m-0"
            disabled={catalog.status === "loading"}
            onClick={catalog.refresh}
          >
            刷新字体列表
          </button>
        )}
      </Row>
    </>
  );
}
