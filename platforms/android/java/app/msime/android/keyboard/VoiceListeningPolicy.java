package app.msime.android;

import java.util.function.Predicate;

/**
 * 键盘聆听面板的排布、音量平滑和中间文字裁剪，不碰 View，JVM 冒烟直接测。iOS 的 `KeyboardVoicePanelPolicy` 和 HarmonyOS 的 `VoiceListeningPolicy` 用同一组数值，三端面板长得一样、动得一样。
 *
 * <p>光圈以前不占排布空间：面板只按圆盘居中，脉冲环和音量光圈画在圆盘外面，面板一矮就伸出视图上沿被裁掉，贴着工具栏。现在圆盘连同光圈的最大外扩一起算进一个方框，方框放不下时先少留一行文字、再藏掉提示行，最后才缩小圆盘，光圈满音量时也在面板里面。面板可用高度不含各入口自己的操作按钮区。
 */
public final class VoiceListeningPolicy {
    /** 圆盘最大直径。可用高度不足时优先保留文字并缩小指示图。 */
    public static final float ORB_MAX_DP = 64f;
    public static final float ORB_MIN_DP = 40f;
    /** 光圈满音量时超出圆盘的宽度占圆盘直径的比例：64 dp 的圆盘外扩 16 dp，方框边长是直径的 1.5 倍。 */
    public static final float HALO_RATIO = .25f;
    /** 中间文字要两行时圆盘至少还有这么大，否则只留一行。 */
    public static final float TWO_LINE_MIN_ORB_DP = 52f;
    public static final float PAD_TOP_DP = 4f;
    public static final float GAP_INDICATOR_TITLE_DP = 6f;
    public static final float GAP_TITLE_HINT_DP = 4f;
    public static final float PAD_BOTTOM_DP = 8f;
    /** 安静时光圈也露出最大外扩的这一部分，看得出在听；开了「移除动画」时光圈就停在这里。 */
    public static final float IDLE = .3f;
    /** 音量变大时跟上的时间常数，比回落快得多：说话一出声光圈就涨，停下来慢慢收。 */
    public static final float ATTACK_MS = 60f;
    public static final float RELEASE_MS = 320f;
    /** 显示值离目标不到这么多就当已经到位，停掉逐帧重绘。 */
    public static final float SETTLE = .01f;
    /** 一帧最多按这么长推进，暂停很久后的第一帧不会一下跳到底。 */
    public static final float MAX_STEP_MS = 100f;
    public static final float OUTER_ALPHA = .16f;
    public static final float INNER_ALPHA = .26f;
    public static final String ELLIPSIS = "…";

    private VoiceListeningPolicy() { }

    /**
     * 一次排布的结果，长度都是像素。{@code orb} 是圆盘直径，{@code halo} 是光圈满音量时超出圆盘的宽度，{@code titleLines} 是标题位预留的行数，{@code showsHint} 为 false 时不画提示行，{@code top} 是整块内容在面板里往下让出的距离（居中后多出来的一半）。
     */
    public record Plan(float orb, float halo, int titleLines, boolean showsHint, float top) {
        public float indicatorCenterY(float scale) {
            return top + PAD_TOP_DP * scale + halo + orb / 2f;
        }

        public float titleTop(float scale) {
            return top + (PAD_TOP_DP + GAP_INDICATOR_TITLE_DP) * scale + orb + 2f * halo;
        }
    }

    /**
     * 按钮行上面那块区域的排布：{@code height} 不含按钮行和它周围的边距，{@code titleLine}、{@code hintLine} 是标题和提示一行的高度，{@code scale} 是每 dp 的像素数。依次试「两行标题加提示」「一行标题加提示」「一行标题不带提示」，取第一个圆盘不小于 {@link #ORB_MIN_DP} 的；两行时圆盘还要不小于 {@link #TWO_LINE_MIN_ORB_DP}。都放不下时圆盘先停在最小值、光圈收窄，再不够才把圆盘缩到剩下的高度。
     *
     * <p>标题位总按预留的行数算，中间文字从一行换到两行时圆盘不会跟着变大变小；只占一行时空出来的那行落在提示下面。
     */
    public static Plan plan(float height, float titleLine, float hintLine, float scale) {
        float available = Math.max(0f, height);
        float orbMax = ORB_MAX_DP * scale;
        float orbMin = ORB_MIN_DP * scale;
        int[] lines = {2, 1, 1};
        boolean[] hints = {true, true, false};
        for (int index = 0; index < lines.length; index++) {
            float fixed = fixed(lines[index], hints[index], titleLine, hintLine, scale);
            float orb = Math.min(orbMax, (available - fixed) / (1f + 2f * HALO_RATIO));
            if (lines[index] == 2 && orb < TWO_LINE_MIN_ORB_DP * scale) continue;
            if (orb >= orbMin) return placed(available, orb, orb * HALO_RATIO, lines[index], hints[index], fixed);
        }
        float fixed = fixed(1, false, titleLine, hintLine, scale);
        float box = Math.max(0f, available - fixed);
        if (box >= orbMin) {
            return placed(available, orbMin, Math.min((box - orbMin) / 2f, orbMin * HALO_RATIO), 1, false, fixed);
        }
        return placed(available, box, 0f, 1, false, fixed);
    }

    private static float fixed(int lines, boolean hint, float titleLine, float hintLine, float scale) {
        float value = (PAD_TOP_DP + GAP_INDICATOR_TITLE_DP + PAD_BOTTOM_DP) * scale + lines * titleLine;
        return hint ? value + GAP_TITLE_HINT_DP * scale + hintLine : value;
    }

    private static Plan placed(float height, float orb, float halo, int lines, boolean hint, float fixed) {
        float used = orb + 2f * halo + fixed;
        return new Plan(orb, halo, lines, hint, Math.max(0f, (height - used) / 2f));
    }

    /** 下一帧显示的音量：按经过的时间指数逼近目标，变大用 {@link #ATTACK_MS}，变小用 {@link #RELEASE_MS}；离目标不到 {@link #SETTLE} 时直接到位。目标取 0–1，NaN 当作安静。 */
    public static float step(float shown, float target, float elapsedMs) {
        float goal = clamp(target);
        float elapsed = Float.isNaN(elapsedMs) ? 0f : Math.max(0f, Math.min(MAX_STEP_MS, elapsedMs));
        float tau = goal > shown ? ATTACK_MS : RELEASE_MS;
        float next = shown + (goal - shown) * (float) (1.0 - Math.exp(-elapsed / tau));
        return Math.abs(goal - next) < SETTLE ? goal : next;
    }

    /** 显示值已经到位，不必再逐帧重绘。 */
    public static boolean settled(float shown, float target) {
        return Math.abs(clamp(target) - shown) < SETTLE;
    }

    /** 外圈光圈超出圆盘的半径：安静时是满外扩的 {@link #IDLE}，满音量时是 {@code halo}；内圈取它的一半。 */
    public static float spread(float shown, float halo) {
        return halo * (IDLE + (1f - IDLE) * clamp(shown));
    }

    /** 0–1，NaN 当作 0。 */
    public static float clamp(float value) {
        if (Float.isNaN(value)) return 0f;
        return Math.max(0f, Math.min(1f, value));
    }

    /** 中间文字去掉首尾空白，连续的空白（含换行）并成一个空格。 */
    public static String normalized(String text) {
        if (text == null) return "";
        StringBuilder out = new StringBuilder(text.length());
        boolean space = false;
        for (int index = 0; index < text.length(); ) {
            int point = text.codePointAt(index);
            index += Character.charCount(point);
            if (Character.isWhitespace(point) || Character.isSpaceChar(point)) {
                space = out.length() > 0;
                continue;
            }
            if (space) out.append(' ');
            space = false;
            out.appendCodePoint(point);
        }
        return out.toString();
    }

    /**
     * 放不下时从开头省略，留住最新说的那段：整段放得下就原样返回，否则找最短的省略，返回「…」加上放得下的最长后缀；按码点切，不拆代理对。一个字也放不下时返回「…」加最后一个字。
     */
    public static String fitTail(String text, Predicate<String> fits) {
        String value = normalized(text);
        if (value.isEmpty() || fits.test(value)) return value;
        int count = value.codePointCount(0, value.length());
        int low = 1;
        int high = count - 1;
        int best = Math.max(0, count - 1);
        while (low <= high) {
            int middle = (low + high) >>> 1;
            if (fits.test(ELLIPSIS + value.substring(value.offsetByCodePoints(0, middle)))) {
                best = middle;
                high = middle - 1;
            } else {
                low = middle + 1;
            }
        }
        return ELLIPSIS + value.substring(value.offsetByCodePoints(0, best));
    }
}
