import {
  defaultCandidateEnglishFont,
  defaultCandidateFallbackFonts,
  defaultCandidateFontFamily,
  validFontFamily,
  type CandidateFontPreferences,
} from "./candidate-font-family";
import { useFontCatalog, type FontCatalogReader } from "./font-catalog";
import { FontFamilyInput } from "./font-family-input";
import { Row } from "../core/platform-controls";

/** The fallback list sits under its row, inset like the row's content. */
const fallbackBlock = "min-w-0 [padding:var(--p-row-pad)] [&>.candidate-fallback-list]:m-0";

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
  const fonts = value.candidate_fallback_fonts ?? [...defaultCandidateFallbackFonts];
  const englishFontValue =
    value.candidate_english_font ??
    (windows
      ? defaultCandidateEnglishFont
      : (value.candidate_font_family ?? defaultCandidateFontFamily));
  const surfaceName = mobile ? "候选栏" : "候选窗";
  const move = (index: number, delta: number) => {
    const next = [...fonts];
    [next[index], next[index + delta]] = [next[index + delta], next[index]];
    onChange({ candidate_fallback_fonts: next });
  };
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
  // Rows of the page's 字体 group: each font is a row with its input on the trailing edge, and the fallback list is a block under its row.
  return (
    <>
      {englishFont && (
        <Row
          title={`${surfaceName}英文字体`}
          description={`优先用于候选和预编辑；缺字后依次使用补充字体，不限英文输入模式。${windows ? "保存后自动应用。" : "未设置时跟随候选主字体。"}`}
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
            className="secondary"
            disabled={catalog.status === "loading"}
            onClick={catalog.refresh}
          >
            刷新字体列表
          </button>
        )}
      </Row>
      <Row
        title={`${surfaceName}补充字体`}
        description="主字体缺字时依次回落，最后使用系统字体。最多 32 项。"
      >
        <button
          type="button"
          className="secondary"
          disabled={fonts.length >= 32 || fonts.some((font) => !validFontFamily(font))}
          onClick={() => onChange({ candidate_fallback_fonts: [...fonts, ""] })}
        >
          添加补充字体
        </button>
      </Row>
      {fonts.length > 0 && (
        <div className={fallbackBlock}>
          <div className="candidate-fallback-list" role="group" aria-label="补充字体回落顺序">
            {fonts.map((font, index) => (
              <div className="candidate-fallback-row" key={index}>
                <FontFamilyInput
                  label={`补充字体 ${index + 1}`}
                  value={font}
                  fonts={catalog.fonts}
                  enabled={!!readFonts}
                  ready={catalog.status === "ready"}
                  request={catalog.request}
                  excluded={fonts.filter((_, position) => position !== index)}
                  onChange={(next) =>
                    onChange({
                      candidate_fallback_fonts: fonts.map((item, position) =>
                        position === index ? next : item,
                      ),
                    })
                  }
                />
                <button
                  type="button"
                  className="secondary"
                  aria-label={`上移补充字体 ${index + 1}`}
                  disabled={index === 0}
                  onClick={() => move(index, -1)}
                >
                  ↑
                </button>
                <button
                  type="button"
                  className="secondary"
                  aria-label={`下移补充字体 ${index + 1}`}
                  disabled={index === fonts.length - 1}
                  onClick={() => move(index, 1)}
                >
                  ↓
                </button>
                <button
                  type="button"
                  className="secondary"
                  aria-label={`移除补充字体 ${index + 1}`}
                  onClick={() =>
                    onChange({
                      candidate_fallback_fonts: fonts.filter((_, position) => position !== index),
                    })
                  }
                >
                  移除
                </button>
              </div>
            ))}
          </div>
        </div>
      )}
    </>
  );
}
