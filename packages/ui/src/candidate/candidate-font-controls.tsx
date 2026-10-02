import {
  defaultCandidateEnglishFont,
  defaultCandidateFontFamily,
  type CandidateFontPreferences,
} from "./candidate-font-family";
import { candidateMainFontPatch } from "./candidate-font-presets";
import { useFontCatalog, type FontCatalogReader } from "./font-catalog";
import { FontFamilyInput } from "./font-family-input";
import { ActionButton } from "../core/action-button";
import { Row } from "../core/platform-controls";

/**
 * The candidate font rows. `candidate_fallback_fonts` has no editor here on purpose: the 候选字体 presets and, on Windows, the main font write it, every host appends the system font after it, and an ordered list of family names - most of them other platforms' faces, since the document is shared - asked users to make a typesetting decision they had no way to judge.
 */
export function CandidateFontControls({
  value,
  onChange,
  readFonts,
  windows = false,
  englishFont = windows,
}: {
  value: CandidateFontPreferences;
  onChange: (patch: CandidateFontPreferences) => void;
  readFonts?: FontCatalogReader;
  windows?: boolean;
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
  // 该页「字体与大小」组里的各行：每种字体一行，输入控件在行尾；主字体排在最前，因为英文字体会回退到它。
  return (
    <>
      <Row title="主字体">
        <FontFamilyInput
          label="主字体"
          value={value.candidate_font_family ?? defaultCandidateFontFamily}
          fonts={catalog.fonts}
          enabled={!!readFonts}
          ready={catalog.status === "ready"}
          request={catalog.request}
          onChange={(font) => onChange(candidateMainFontPatch(font, windows, value))}
        />
      </Row>
      {englishFont && (
        <Row
          title="英文字体"
          description={`优先用于候选和预编辑；缺字时使用主字体，不限英文输入模式。${windows ? "保存后自动应用。" : "未设置时跟随候选主字体。"}`}
        >
          <FontFamilyInput
            label="英文字体"
            value={englishFontValue}
            fonts={catalog.fonts}
            enabled={!!readFonts}
            ready={catalog.status === "ready"}
            request={catalog.request}
            onChange={(font) => onChange({ candidate_english_font: font })}
          />
        </Row>
      )}
      {/* The live region stays mounted while empty, so the status it later takes is announced. */}
      <Row title="系统字体列表" description={<span role="status">{catalogStatus}</span>}>
        {readFonts && (
          <ActionButton
            action={catalog.refresh}
            className="secondary m-0"
            disabled={catalog.status === "loading"}
            label="刷新字体列表"
          />
        )}
      </Row>
    </>
  );
}
