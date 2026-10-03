#include "CandidateItemLayout.h"

#include <cassert>
#include <cmath>

// The cases platforms/windows/tests/ui/candidate_card_size.cpp asks of candidate_item_layout and candidate_page_layout, asked of the macOS port: both hosts draw the same MeasureItem rule.
namespace
{
bool Near(double value, double expected)
{
    return std::fabs(value - expected) < 0.001;
}

msime::mac::CandidateLayoutMetrics Metrics()
{
    msime::mac::CandidateLayoutMetrics metrics;
    metrics.candidateRow = 32.0;
    metrics.chrome = 30.0;
    metrics.annotationGap = 4.0;
    metrics.annotationLine = 20.0;
    metrics.translationGap = 16.0 * 0.65;
    metrics.translationLine = 20.0;
    return metrics;
}
} // namespace

int main()
{
    using namespace msime::mac;
    const CandidateLayoutMetrics metrics = Metrics();

    // Text that fits keeps one row and nothing wraps; short runs share its line.
    {
        const CandidateItemLayout item = LayoutCandidateItem({60.0, 30.0, 40.0}, 300.0, metrics, false, {});
        assert(!item.textWrapped && Near(item.textWidth, 60.0) && Near(item.textHeight, 32.0));
        assert(!item.annotation.below && Near(item.annotation.x, 64.0));
        assert(!item.translation.below && Near(item.translation.x, 64.0 + 30.0 + metrics.translationGap));
        assert(Near(item.height, 32.0));
    }

    // Over-wide text wraps to the measured height, and the annotation and gloss go below it.
    {
        CandidateRun asked = CandidateRun::annotation;
        double askedWidth = 0.0;
        const CandidateItemLayout item = LayoutCandidateItem({500.0, 30.0, 40.0}, 200.0, metrics, false,
            [&](CandidateRun run, double width) {
                if (run == CandidateRun::text)
                {
                    asked = run;
                    askedWidth = width;
                    return 3.0 * 20.0 + 12.0;
                }
                return 0.0;
            });
        assert(asked == CandidateRun::text && Near(askedWidth, 200.0));
        assert(item.textWrapped && Near(item.textWidth, 200.0) && Near(item.textHeight, 72.0));
        assert(item.annotation.below && Near(item.annotation.y, 72.0) && Near(item.annotation.height, 20.0));
        assert(item.translation.below && Near(item.translation.y, 92.0));
        assert(Near(item.height, 72.0 + 20.0 + 20.0));
    }

    // An annotation that does not fit beside the text moves below, and the gloss follows it even when it would fit.
    {
        const CandidateItemLayout item = LayoutCandidateItem({150.0, 60.0, 10.0}, 200.0, metrics, false, {});
        assert(!item.textWrapped && item.annotation.below && Near(item.annotation.x, 0.0));
        assert(item.translation.below && Near(item.translation.y, 32.0 + 20.0));
    }

    // A vertical gloss stays inline when it fits and wraps below when it does not; the wrapped width is the column.
    {
        const CandidateItemLayout inlineItem = LayoutCandidateItem({60.0, 0.0, 80.0}, 200.0, metrics, false, {});
        assert(!inlineItem.translation.below && Near(inlineItem.height, 32.0));
        double askedWidth = 0.0;
        const CandidateItemLayout wrapped = LayoutCandidateItem({60.0, 0.0, 400.0}, 200.0, metrics, false,
            [&](CandidateRun run, double width) {
                assert(run == CandidateRun::translation);
                askedWidth = width;
                return 44.0;
            });
        assert(wrapped.translation.below && Near(wrapped.translation.width, 200.0) && Near(askedWidth, 200.0));
        assert(Near(wrapped.height, 32.0 + 44.0));
        // A gloss with a line per target language makes an inline row as tall as it is.
        CandidateItemWidths twoLanguages{60.0, 0.0, 80.0, 44.0};
        assert(Near(LayoutCandidateItem(twoLanguages, 200.0, metrics, false, {}).height, 44.0));
    }

    // Horizontal always puts the gloss below, even when it would fit beside the text.
    {
        const CandidateItemLayout item = LayoutCandidateItem({60.0, 0.0, 20.0}, 300.0, metrics, true, {});
        assert(item.translation.below && Near(item.translation.y, 32.0) && Near(item.height, 52.0));
    }

    // A failed, NaN or too small measure still reserves one line; no measure estimates from the single-line width.
    {
        const CandidateItemWidths wide{500.0, 0.0, 0.0};
        assert(Near(LayoutCandidateItem(wide, 200.0, metrics, false, [](CandidateRun, double) { return std::nan(""); }).textHeight, 32.0));
        assert(Near(LayoutCandidateItem(wide, 200.0, metrics, false, [](CandidateRun, double) { return 2.0; }).textHeight, 32.0));
        assert(Near(LayoutCandidateItem(wide, 200.0, metrics, false, {}).textHeight, 3.0 * 20.0));
        const CandidateItemWidths gloss{60.0, 0.0, 500.0};
        assert(Near(LayoutCandidateItem(gloss, 200.0, metrics, true, [](CandidateRun, double) { return -1.0; }).translation.height, 20.0));
    }

    // Natural widths: a vertical row adds the gloss to the line, a horizontal one takes the wider of the two lines.
    {
        const CandidateItemWidths item{60.0, 30.0, 200.0};
        assert(Near(CandidateItemNaturalWidth(item, metrics, false), 60.0 + 4.0 + 30.0 + metrics.translationGap + 200.0 + 30.0));
        assert(Near(CandidateItemNaturalWidth(item, metrics, true), 200.0 + 30.0));
        assert(Near(CandidateItemNaturalWidth({}, metrics, true), 0.0));
    }

    // A vertical page stacks full-width rows at their own heights.
    {
        const std::vector<CandidateItemWidths> items{{60.0}, {900.0}, {40.0}};
        const auto rows = LayoutCandidatePage(items, 230.0, metrics, false,
            [](std::size_t index, CandidateRun, double) { return index == 1 ? 112.0 : 0.0; });
        assert(rows.size() == 3);
        assert(Near(rows[0].y, 0.0) && Near(rows[0].height, 32.0) && Near(rows[0].width, 230.0));
        assert(rows[1].item.textWrapped && Near(rows[1].y, 32.0) && Near(rows[1].height, 112.0));
        assert(Near(rows[2].y, 144.0) && Near(rows[2].height, 32.0));
        assert(Near(CandidatePageHeight(rows), 176.0));
    }

    // A horizontal page runs natural-width columns left to right and breaks the line when the next would pass its end; every column on a line takes the line's tallest height, and the minimum reserves room for a gloss.
    {
        const std::vector<CandidateItemWidths> items{{100.0, 0.0, 20.0}, {50.0}, {80.0}};
        const auto rows = LayoutCandidatePage(items, 250.0, metrics, true, {}, 40.0);
        assert(Near(rows[0].x, 0.0) && Near(rows[0].width, 130.0));
        assert(Near(rows[1].x, 130.0) && Near(rows[1].width, 80.0) && Near(rows[1].y, 0.0));
        assert(Near(rows[0].height, 52.0) && Near(rows[1].height, 52.0));
        assert(Near(rows[2].x, 0.0) && Near(rows[2].y, 52.0) && Near(rows[2].height, 40.0));
        assert(Near(CandidatePageHeight(rows), 92.0));
        // Only a candidate wider than a whole line is narrowed to it, and it wraps inside that column.
        const auto narrowed = LayoutCandidatePage({{600.0}, {50.0}}, 250.0, metrics, true, {});
        assert(Near(narrowed[0].width, 250.0) && narrowed[0].item.textWrapped && Near(narrowed[0].item.textWidth, 220.0));
        assert(Near(narrowed[1].x, 0.0) && Near(narrowed[1].y, narrowed[0].height));
    }

    // Glosses too wide for one line give up room in proportion to what they can spare, never below their candidate's line, so the page stays on one line and the glosses wrap under their text.
    {
        const std::vector<CandidateItemWidths> items{{60.0, 0.0, 200.0}, {50.0, 0.0, 150.0}, {40.0}};
        const auto columns = SingleLineColumns(items, 300.0, metrics);
        assert(columns && Near((*columns)[0], 125.0) && Near((*columns)[1], 105.0) && Near((*columns)[2], 70.0));
        assert(columns->capacity() == items.size());
        const auto rows = LayoutCandidatePage(items, 300.0, metrics, true,
            [](std::size_t, CandidateRun run, double) { return run == CandidateRun::translation ? 40.0 : 0.0; });
        assert(Near(rows[0].x, 0.0) && Near(rows[1].x, 125.0) && Near(rows[2].x, 230.0));
        assert(Near(rows[0].y, 0.0) && Near(rows[1].y, 0.0) && Near(rows[2].y, 0.0));
        assert(Near(rows[0].item.translation.width, 95.0) && Near(rows[0].height, 32.0 + 40.0));
        // Columns that fit keep their natural widths.
        const auto fits = SingleLineColumns(items, 500.0, metrics);
        assert(fits && Near((*fits)[0], 230.0) && Near((*fits)[1], 180.0) && Near((*fits)[2], 70.0));
        // When the candidate lines alone pass the line's end there is no single line to keep.
        assert(!SingleLineColumns(items, 200.0, metrics));
        // The narrowest single line is the candidates' own lines with their chrome; the card grows to it before a page breaks.
        assert(Near(SingleLineMinimumWidth(items, metrics), 90.0 + 80.0 + 70.0));
        assert(SingleLineColumns(items, SingleLineMinimumWidth(items, metrics), metrics));
    }

    // The card is at least 7em of the candidate font; a skin floor only raises it, and the 7em part never passes the screen cap.
    {
        assert(Near(CandidateCardMinimumWidth(16.0, 0.0, 0.0), 112.0));
        assert(Near(CandidateCardMinimumWidth(16.0, 200.0, 0.0), 200.0));
        assert(Near(CandidateCardMinimumWidth(16.0, 50.0, 0.0), 112.0));
        assert(Near(CandidateCardMinimumWidth(16.0, std::nan(""), 0.0), 112.0));
        assert(Near(CandidateCardMinimumWidth(16.0, -30.0, 0.0), 112.0));
        // A skin decoration wider than 7em wins.
        assert(Near(CandidateCardMinimumWidth(16.0, 180.0, 720.0), 180.0));
        assert(Near(CandidateCardMinimumWidth(16.0, 0.0, 90.0), 90.0));
        assert(Near(CandidateCardMinimumWidth(32.0, 0.0, 720.0), 224.0));
        // The skin's own floor is not clamped by the cap.
        assert(Near(CandidateCardMinimumWidth(16.0, 300.0, 90.0), 300.0));
    }
    return 0;
}
