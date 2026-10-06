package app.msime.android;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.RectF;
import android.graphics.Typeface;
import android.text.TextPaint;
import android.text.TextUtils;

/**
 * 键盘内皮肤面板的一格瓷砖：上面是圆角 8 dp、按设计 MiniKb 比例的整副迷你键盘，下面一行 12 sp 的皮肤名。
 *
 * <p>选中时缩略图外一圈 2 dp 的 accent 描边、名字用 accent 加粗；未选中是 1 dp 的 kbHair 描边、名字用 kbFg。这几样颜色由调用方经 {@link #setTileColors} 从当前键盘皮肤传入（面板的配色，而不是瓷砖代表的那个皮肤）；没传时退回瓷砖皮肤自己的颜色。节点描述沿用构造时的标题，调用方可再覆盖。
 */
public final class KeyboardSkinCard extends KeyboardPressButton {
    public static final float TILE_RADIUS_DP = 8f;
    public static final float LABEL_SP = 12f;
    public static final float LABEL_GAP_DP = 6f;

    private final KeyboardSkin skin;
    private final String title;
    private final float density;
    private final KeyboardSkinPreview.TileDrawState tileDrawState;
    private final TextPaint paint = new TextPaint(Paint.ANTI_ALIAS_FLAG);
    private final RectF tile = new RectF();
    private final RectF outline = new RectF();
    private String fittedTitle;
    private float fittedTitleWidth = Float.NaN;
    private float fittedTitleSize = Float.NaN;
    private boolean fittedTitleBold;
    private boolean themed;
    private int accentColor;
    private int hairlineColor;
    private int labelColor;
    private Integer splitStart;
    private Integer splitEnd;
    private KeyboardSkinPreview.SplitDrawState splitDrawState;

    public KeyboardSkinCard(Context context, KeyboardSkin skin, String title) {
        super(context);
        this.skin = skin;
        this.title = title;
        density = KeyboardGeometry.density(context);
        tileDrawState = new KeyboardSkinPreview.TileDrawState(skin, density);
        setAllCaps(false);
        setBackground(null);
        setText(null);
        setWillNotDraw(false);
        setContentDescription(title);
    }

    /** 面板配色：选中描边与选中名字（accent）、未选中描边（kbHair）、未选中名字（kbFg）。 */
    public void setTileColors(int accent, int hairline, int label) {
        themed = true;
        accentColor = accent;
        hairlineColor = hairline;
        labelColor = label;
        invalidate();
    }

    /** 「跟随系统」那格画 135° 对半分的浅深两色，而不是某一个皮肤的缩略图。 */
    public void setSplitPreview(int light, int dark) {
        splitStart = light;
        splitEnd = dark;
        if (splitDrawState == null) splitDrawState = new KeyboardSkinPreview.SplitDrawState();
        invalidate();
    }

    public KeyboardSkin skin() { return skin; }

    @Override protected void onDraw(Canvas canvas) {
        float labelSize = KeyboardGeometry.keySp(getContext(), LABEL_SP);
        paint.setTextSize(labelSize);
        Paint.FontMetrics metrics = paint.getFontMetrics();
        float labelHeight = metrics.descent - metrics.ascent;
        float ring = 2 * density;
        // 缩略图按设计 MiniKb 的 390:292 由宽度定高，名字紧贴在下方；格子更高时多出的空白留在名字下面，不拉伸缩略图。
        float available = getHeight() - labelHeight - LABEL_GAP_DP * density - ring * 2;
        float tileHeight = Math.min(available,
            (getWidth() - ring * 2) * KeyboardSkinPreview.MINI_HEIGHT / KeyboardSkinPreview.MINI_WIDTH);
        tile.set(ring, ring, getWidth() - ring, ring + tileHeight);
        if (tile.width() <= 0 || tile.height() <= 0) return;
        float radius = TILE_RADIUS_DP * density;
        if (splitStart != null && splitEnd != null) {
            KeyboardSkinPreview.drawSplit(canvas, tile, radius, splitStart, splitEnd,
                splitDrawState);
        } else {
            KeyboardSkinPreview.drawTile(canvas, tile, radius, skin, density, tileDrawState);
        }
        int accent = themed ? accentColor : Color.parseColor(skin.accent());
        int hairline = themed ? hairlineColor : Color.argb(31, 0, 0, 0);
        int label = themed ? labelColor : Color.parseColor(skin.keyForeground());
        boolean selected = isSelected();
        paint.setStyle(Paint.Style.STROKE);
        float stroke = (selected ? 2 : 1) * density;
        paint.setStrokeWidth(stroke);
        paint.setColor(selected ? accent : hairline);
        // 描边整条落在缩略图外侧，不压住缩略图本身。
        outline.set(tile);
        outline.inset(-stroke / 2f, -stroke / 2f);
        canvas.drawRoundRect(outline, radius + stroke / 2f, radius + stroke / 2f, paint);
        paint.setStyle(Paint.Style.FILL);
        paint.setColor(selected ? accent : label);
        paint.setTypeface(selected ? Typeface.DEFAULT_BOLD : Typeface.DEFAULT);
        paint.setTextAlign(Paint.Align.CENTER);
        // 皮肤名放不下时以「…」结尾，不悄悄丢掉末尾的字。
        boolean bold = selected;
        float width = BoundsPolicy.nonNegative(getWidth());
        if (fittedTitle == null || fittedTitleWidth != width || fittedTitleSize != labelSize
                || fittedTitleBold != bold) {
            fittedTitle = TextUtils.ellipsize(title == null ? "" : title, paint, width,
                TextUtils.TruncateAt.END).toString();
            fittedTitleWidth = width;
            fittedTitleSize = labelSize;
            fittedTitleBold = bold;
        }
        canvas.drawText(fittedTitle, getWidth() / 2f, tile.bottom + ring + LABEL_GAP_DP * density
            - metrics.ascent, paint);
    }
}
