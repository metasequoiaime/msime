package app.msime.android;

import java.util.Collections;
import java.util.EnumMap;
import java.util.List;
import java.util.Map;

/**
 * 一个键组里每个位置**可能**出现的全部文字：按当前设置可能画出来的，不只是此刻画着的（例如 14/18 键分词键的「辅」在不组字时也算在内）。键组按它预留空间，所以提示出现、消失只重画，不重排。
 *
 * <p>位置没有文字（或不在表里）就当这个键组不用它，不为它留空间。
 */
public record KeyFaceDomain(Map<KeyFaceSlot, List<String>> texts) {
    /** 量字的基准字号（像素）：内容域只在这个字号下量一次，其他字号按比例换算。 */
    public static final float REFERENCE_PX = 100f;

    public KeyFaceDomain {
        EnumMap<KeyFaceSlot, List<String>> copy = new EnumMap<>(KeyFaceSlot.class);
        for (Map.Entry<KeyFaceSlot, List<String>> entry : texts.entrySet()) {
            List<String> values = entry.getValue().stream().filter(text -> text != null && !text.isEmpty()).toList();
            if (!values.isEmpty()) copy.put(entry.getKey(), values);
        }
        texts = Collections.unmodifiableMap(copy);
    }

    public boolean has(KeyFaceSlot slot) { return texts.containsKey(slot); }

    /**
     * 一个位置在 {@link #REFERENCE_PX} 字号下的上界：最宽的排版宽度、最宽的墨迹，以及所有文字墨迹合起来的上沿和下沿（相对基线，上沿为负）。
     */
    public record SlotMetrics(float width, float inkWidth, float inkTop, float inkBottom) {
        public float inkHeight() { return inkBottom - inkTop; }

        /** 按字号 {@code sizePx} 换算的排版宽度。 */
        public float width(float sizePx) { return width * sizePx / REFERENCE_PX; }

        public float inkWidth(float sizePx) { return inkWidth * sizePx / REFERENCE_PX; }

        public float inkTop(float sizePx) { return inkTop * sizePx / REFERENCE_PX; }

        public float inkBottom(float sizePx) { return inkBottom * sizePx / REFERENCE_PX; }

        public float inkHeight(float sizePx) { return inkHeight() * sizePx / REFERENCE_PX; }
    }

    /** 量一遍内容域。{@code pairGapEm} 见 {@link KeyFaceTemplate#pairGapEm}：两个字符的主标签按「左字 + 间距 + 右字」算宽度。 */
    public Map<KeyFaceSlot, SlotMetrics> measure(KeyFaceMeasurer measurer, float pairGapEm) {
        EnumMap<KeyFaceSlot, SlotMetrics> metrics = new EnumMap<>(KeyFaceSlot.class);
        for (Map.Entry<KeyFaceSlot, List<String>> entry : texts.entrySet()) {
            KeyFaceSlot slot = entry.getKey();
            float width = 0, inkWidth = 0, inkTop = 0, inkBottom = 0;
            for (String text : entry.getValue()) {
                KeyFaceMeasurer.Ink ink = measurer.ink(slot, text, REFERENCE_PX);
                float advance = slot == KeyFaceSlot.MAIN ? mainWidth(measurer, text, REFERENCE_PX, pairGapEm)
                    : measurer.advance(slot, text, REFERENCE_PX);
                width = Math.max(width, advance);
                inkWidth = Math.max(inkWidth, slot == KeyFaceSlot.MAIN && pairs(text, pairGapEm) ? advance : ink.width());
                inkTop = Math.min(inkTop, ink.top());
                inkBottom = Math.max(inkBottom, ink.bottom());
            }
            metrics.put(slot, new SlotMetrics(width, inkWidth, inkTop, inkBottom));
        }
        return metrics;
    }

    /** 主标签的排版宽度：两个字符且模板给了间距时按「左字 + 间距 + 右字」，否则整串。绘制用同一规则。 */
    public static float mainWidth(KeyFaceMeasurer measurer, String text, float sizePx, float pairGapEm) {
        if (!pairs(text, pairGapEm)) return measurer.advance(KeyFaceSlot.MAIN, text, sizePx);
        return measurer.advance(KeyFaceSlot.MAIN, text.substring(0, 1), sizePx) + pairGapEm * sizePx
            + measurer.advance(KeyFaceSlot.MAIN, text.substring(1), sizePx);
    }

    /** 这段主标签是否按两个字符分开排。 */
    public static boolean pairs(String text, float pairGapEm) {
        return pairGapEm > 0 && text.length() == 2 && text.codePointCount(0, 2) == 2;
    }
}
