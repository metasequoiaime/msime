package app.msime.android;

import java.util.ArrayList;
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
            // Android 9 没有 Stream.toList（API 34 起才有，D8 不补），用普通循环。
            List<String> values = new ArrayList<>();
            for (String text : entry.getValue())
                if (text != null && !text.isEmpty() && !values.contains(text)) values.add(text);
            if (!values.isEmpty()) copy.put(entry.getKey(), Collections.unmodifiableList(values));
        }
        texts = Collections.unmodifiableMap(copy);
    }

    public boolean has(KeyFaceSlot slot) { return texts.containsKey(slot); }

    /** 这个位置的内容域里有没有 {@code text}；空串和 {@code null} 算有（不画就不用留位置）。 */
    public boolean contains(KeyFaceSlot slot, String text) {
        return text == null || text.isEmpty() || texts.getOrDefault(slot, List.of()).contains(text);
    }

    /** 加上一段文字后的内容域。 */
    public KeyFaceDomain with(KeyFaceSlot slot, String text) {
        if (contains(slot, text)) return this;
        EnumMap<KeyFaceSlot, List<String>> copy = new EnumMap<>(KeyFaceSlot.class);
        copy.putAll(texts);
        List<String> values = new ArrayList<>(texts.getOrDefault(slot, List.of()));
        values.add(text);
        copy.put(slot, values);
        return new KeyFaceDomain(copy);
    }

    /**
     * 一个位置在 {@link #REFERENCE_PX} 字号下的上界：最宽的排版宽度、最宽的墨迹，所有文字墨迹合起来的上沿和下沿（相对基线，上沿为负），以及字体的上行线（{@link KeyFaceMeasurer#ascent}，为负）。
     */
    public record SlotMetrics(float width, float inkWidth, float inkTop, float inkBottom, float ascent) {
        public float inkHeight() { return inkBottom - inkTop; }

        /** 按字号 {@code sizePx} 换算的排版宽度。 */
        public float width(float sizePx) { return width * sizePx / REFERENCE_PX; }

        public float inkWidth(float sizePx) { return inkWidth * sizePx / REFERENCE_PX; }

        public float inkTop(float sizePx) { return inkTop * sizePx / REFERENCE_PX; }

        public float inkBottom(float sizePx) { return inkBottom * sizePx / REFERENCE_PX; }

        public float inkHeight(float sizePx) { return inkHeight() * sizePx / REFERENCE_PX; }

        public float ascent(float sizePx) { return ascent * sizePx / REFERENCE_PX; }

        /**
         * 竖直居中时要占的高度：按整个墨迹居中时就是墨迹高；按基线以上的墨迹居中（{@link KeyFaceTemplate#capAligned}）时，基线以下的部分挂在居中的那段下面，所以上下各要留出一份。
         */
        public float alignedHeight(float sizePx, boolean capAligned) {
            if (!capAligned) return inkHeight(sizePx);
            return (-Math.min(inkTop, 0) + 2 * Math.max(inkBottom, 0)) * sizePx / REFERENCE_PX;
        }
    }

    /** 量一遍内容域。{@code pairGapEm} 见 {@link KeyFaceTemplate#pairGapEm}：两个字符的主标签按「左字 + 间距 + 右字」算宽度。 */
    public Map<KeyFaceSlot, SlotMetrics> measure(KeyFaceMeasurer measurer, float pairGapEm) {
        return measure(measurer, pairGapEm, null);
    }

    /**
     * 同上；{@code glyphs} 不为 {@code null} 时主标签按每段文字自己的墨迹居中（{@link KeyFaceTemplate.Glyphs}）：每段按自己的放大倍数量墨迹，宽度取最宽的墨迹，墨迹上下沿以基线为中线、上下各取最高的墨迹的一半（登记的字再加上它往下挪的距离，上下对称地留）。按这个框居中的主标签，是每个键把自己的墨迹居中（再往下挪）后都落得进去的范围，求解器照常用它判断放不放得下、碰不碰提示。
     */
    public Map<KeyFaceSlot, SlotMetrics> measure(KeyFaceMeasurer measurer, float pairGapEm, KeyFaceTemplate.Glyphs glyphs) {
        EnumMap<KeyFaceSlot, SlotMetrics> metrics = new EnumMap<>(KeyFaceSlot.class);
        for (Map.Entry<KeyFaceSlot, List<String>> entry : texts.entrySet()) {
            KeyFaceSlot slot = entry.getKey();
            if (slot == KeyFaceSlot.MAIN && glyphs != null) {
                float wide = 0, half = 0;
                for (String text : entry.getValue()) {
                    KeyFaceMeasurer.Ink ink = measurer.ink(slot, text, REFERENCE_PX * glyphs.scale(text));
                    wide = Math.max(wide, ink.width());
                    half = Math.max(half, ink.height() / 2f + glyphs.drop(text, REFERENCE_PX));
                }
                metrics.put(slot, new SlotMetrics(wide, wide, -half, half, measurer.ascent(slot, REFERENCE_PX)));
                continue;
            }
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
            metrics.put(slot, new SlotMetrics(width, inkWidth, inkTop, inkBottom, measurer.ascent(slot, REFERENCE_PX)));
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
