package app.msime.android;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.drawable.Drawable;
import android.text.style.ReplacementSpan;
import android.util.SparseArray;

/**
 * 候选字后面的来源标记（{@link CandidateSourceMark}）：单色矢量图，着候选释义的颜色，跟随皮肤和选中状态。
 *
 * <p>标记占一个替换字符，宽度是一小段间隔加图形本身。行高沿用候选字体的度量，所以带标记和不带标记的候选一样高，{@code CandidateStripSizing} 按样例文字量出的候选栏高度不受影响；彩色 emoji 走回退字体时行距会被撑高，这也是这里不用 emoji 的原因之一。
 */
public final class CandidateSourceMarkSpan extends ReplacementSpan {
    /** 补在候选字后面、代替图形的字符；读屏念的是按钮的 contentDescription，不念它。 */
    public static final String PLACEHOLDER = "￼";
    /** 图形边长相对候选字号。24 格画布里图形约占六成，画出来比汉字小一号，与 iOS 用说明文字字号画符号相当。 */
    static final float SIZE = 0.8f;
    /** 候选字与图形之间的间隔，相对候选字号。 */
    static final float GAP = 0.12f;

    // 三个图形各缓存一份可着色的副本；候选只在主线程绘制，每次画之前设好颜色。
    private static final SparseArray<Drawable> DRAWABLES = new SparseArray<>();

    private final Drawable drawable;
    private final int color;

    private CandidateSourceMarkSpan(Drawable drawable, int color) {
        this.drawable = drawable;
        this.color = color;
    }

    /** {@code glyph} 为 NONE 时返回 null，调用方不补占位字符。 */
    public static CandidateSourceMarkSpan of(Context context, CandidateSourceMark.Glyph glyph, int color) {
        String name = switch (glyph) {
            case CLOUD -> "ic_candidate_source_cloud";
            case ROBOT -> "ic_candidate_source_robot";
            case WHALE -> "ic_candidate_source_whale";
            case NONE -> null;
        };
        if (name == null) return null;
        Drawable drawable = DRAWABLES.get(glyph.ordinal());
        if (drawable == null) {
            Drawable loaded = context.getDrawable(resource(context, name));
            if (loaded == null) throw new IllegalStateException("Missing candidate source mark: " + name);
            drawable = loaded.mutate();
            DRAWABLES.put(glyph.ordinal(), drawable);
        }
        return new CandidateSourceMarkSpan(drawable, color);
    }

    /**
     * 在 {@code label} 的 {@code at} 处（候选字之后、释义换行之前）补一个占位字符并挂上标记；{@code mark} 为 NONE 时原样返回。
     * 插入点之后的样式区间随之后移，候选字自己的颜色区间是 EXCLUSIVE_EXCLUSIVE，不会把标记吞进去。
     */
    public static CharSequence insert(Context context, CharSequence label, int at, CandidateSourceMark.Glyph mark,
            int color) {
        CandidateSourceMarkSpan span = of(context, mark, color);
        if (span == null || at < 0 || at > label.length()) return label;
        android.text.SpannableStringBuilder result = new android.text.SpannableStringBuilder(label);
        result.insert(at, PLACEHOLDER);
        result.setSpan(span, at, at + PLACEHOLDER.length(), android.text.Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
        return result;
    }

    // 键盘这一侧的类不引用 R（宿主检查不带 R 编译它们），按名字找资源；换了包名的测试包里资源仍在原包名下，与输入法主题的查找同一个回退。
    private static int resource(Context context, String name) {
        android.content.res.Resources resources = context.getResources();
        int id = resources.getIdentifier(name, "drawable", context.getPackageName());
        if (id == 0) id = resources.getIdentifier(name, "drawable", "app.msime.android");
        if (id == 0) throw new IllegalStateException("Missing candidate source mark: " + name);
        return id;
    }

    @Override public int getSize(Paint paint, CharSequence text, int start, int end, Paint.FontMetricsInt metrics) {
        if (metrics != null) paint.getFontMetricsInt(metrics);
        return (int) Math.ceil(paint.getTextSize() * (GAP + SIZE));
    }

    @Override public void draw(Canvas canvas, CharSequence text, int start, int end,
            float x, int top, int y, int bottom, Paint paint) {
        float textSize = paint.getTextSize();
        float size = textSize * SIZE;
        float left = x + textSize * GAP;
        // 竖直方向对准字面的中线（ascent 与 descent 的中点），与汉字的视觉中心对齐。
        float centre = y + (paint.ascent() + paint.descent()) * 0.5f;
        float glyphTop = centre - size * 0.5f;
        drawable.setBounds(Math.round(left), Math.round(glyphTop), Math.round(left + size), Math.round(glyphTop + size));
        drawable.setTint(color);
        drawable.setAlpha(paint.getAlpha());
        drawable.draw(canvas);
    }
}
