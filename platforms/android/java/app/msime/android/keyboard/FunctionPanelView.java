package app.msime.android;

import app.msime.android.LayoutPolicy;


import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.RectF;
import android.graphics.Typeface;
import android.graphics.drawable.Drawable;
import android.os.Build;
import android.text.TextPaint;
import android.text.TextUtils;
import android.view.View;
import android.widget.Button;
import android.widget.LinearLayout;
import app.msime.android.KeyboardGeometry;
import java.util.ArrayList;
import java.util.List;

/**
 * 功能菜单：横向分页的条目，下方页点。每页四列，行数见 {@link #setPhoneLandscape}：手机竖屏和平板三行，手机横屏两行；网格按面板量到的高度排（{@link PagedTileGrid#setPickerFit}），放不下原尺寸时磁贴整体缩小，不让面板竖向滚动。
 *
 * <p>条目没有底色：30 dp 图标区与常规字重标签，间距 7 dp，条目高 52 dp。字形条目画方框。开启状态用强调色和图标右下角的勾标表达。条目比 52 dp 矮时（{@link PagedTileGrid#pickerGrid} 缩小了磁贴），图标区、图标、字形框、文字、间距和勾标按同一比例缩小，点按区域仍是整个格子。
 *
 * <p>这个视图**不设置 alpha**：不可用状态的视觉由调用方经 SVC 的 `applyToolCardState` 处理，这里只把状态写进 stateDescription（已开启 / 已关闭 / 不可用）和 `isSelected()`。颜色全部由调用方从皮肤传入。
 */
public final class FunctionPanelView extends LinearLayout {
    /** 条目的开关状态。 */
    public enum State { ON, OFF, UNAVAILABLE, NONE }

    /** 一个条目：标签、无障碍描述、图标（或字形），以及点击与长按回调。 */
    public static final class Entry {
        final String label;
        final String description;
        final KeyboardIconPaths.Icon icon;
        final String glyph;
        final Runnable action;
        final Runnable longPress;

        private Entry(String label, String description, KeyboardIconPaths.Icon icon, String glyph,
                Runnable action, Runnable longPress) {
            this.label = label;
            this.description = description;
            this.icon = icon;
            this.glyph = glyph;
            this.action = action;
            this.longPress = longPress;
        }

        /** 画描边图标的条目。{@code description} 为空时用标签。 */
        public static Entry icon(String label, String description, KeyboardIconPaths.Icon icon,
                Runnable action, Runnable longPress) {
            return new Entry(label, description, icon, null, action, longPress);
        }

        /** 画方框字形的条目（全、，、≈、繁）。 */
        public static Entry glyph(String label, String description, String glyph,
                Runnable action, Runnable longPress) {
            return new Entry(label, description, null, glyph, action, longPress);
        }
    }

    public static final float ITEM_HEIGHT_DP = 52f;
    private static final float PADDING_TOP_DP = 10f;
    private static final float PADDING_BOTTOM_DP = 4f;
    private static final float DOTS_TOP_MARGIN_DP = 8f;
    /** 网格以外的高度：上下内边距和页点（含它的上边距），不含外面容器的内边距。 */
    public static final float CHROME_DP = PADDING_TOP_DP + PADDING_BOTTOM_DP + DOTS_TOP_MARGIN_DP
        + KeyboardPagerDots.DOT_DP;

    private final PagedTileGrid grid;
    private final KeyboardPagerDots dots;
    private final ArrayList<Tile> tiles = new ArrayList<>(FunctionPanelModel.items().size());
    private int foreground = Color.BLACK;
    private int accent = Color.BLUE;
    private int panelBackground = Color.WHITE;

    public FunctionPanelView(Context context) {
        super(context);
        setOrientation(VERTICAL);
        setContentDescription("更多工具");
        LayoutPolicy.setPaddingDp(this, context, 0, PADDING_TOP_DP, 0, PADDING_BOTTOM_DP);
        grid = new PagedTileGrid(context);
        grid.setGrid(FunctionPanelModel.COLUMNS, FunctionPanelModel.ROWS);
        grid.setSpacing(ITEM_HEIGHT_DP, PagedTileGrid.PICKER_THREE_ROW_GAP_DP, 4f, 4f);
        grid.setPickerFit(ITEM_HEIGHT_DP, false);
        dots = new KeyboardPagerDots(context);
        // 网格按量到的高度换行数时页数会变，页点的个数跟着改。
        grid.setOnPageChangeListener((page, count) -> {
            dots.setCount(count);
            dots.setActive(page, true);
        });
        LinearLayout.LayoutParams gridParams = LayoutPolicy.weightedWidthParams(1f);
        addView(grid, gridParams);
        // 页点占满一行、自己居中画：页数随行数变时只需重画，不必重新量宽度。
        LinearLayout.LayoutParams dotParams = LayoutPolicy.matchWidthWrapParams();
        dotParams.topMargin = DimensionPolicy.pixels(context, DOTS_TOP_MARGIN_DP);
        addView(dots, dotParams);
    }

    /** 文字与图标色（kbFg）、开启色（accent）、面板底色（kbBg，用于 ✓ 角标的圆环）、页点非活动色（kbHair）。 */
    public void setColors(int fg, int accentColor, int kbBackground, int hairline) {
        foreground = fg;
        accent = accentColor;
        panelBackground = kbBackground;
        dots.setColors(accentColor, hairline);
        for (Tile tile : tiles) tile.invalidate();
    }

    /** 手机横屏两行，其余三行（{@link PagedTileGrid#pickerGrid}）；行高、行距和是否退成两行由网格按量到的高度定，变了时停在的页超出新页数就退到最后一页，页点跟着重算。 */
    public void setPhoneLandscape(boolean phoneLandscape) {
        grid.setPickerFit(ITEM_HEIGHT_DP, phoneLandscape);
    }

    /** 换掉全部条目；状态都重置为 {@link State#NONE}。 */
    public void setEntries(List<Entry> entries) {
        grid.removeAllViews();
        tiles.clear();
        tiles.ensureCapacity(entries.size());
        for (Entry entry : entries) {
            Tile tile = new Tile(getContext(), this, entry);
            tiles.add(tile);
            grid.addView(tile);
        }
        dots.setCount(grid.pageCount());
        dots.setActive(grid.page(), false);
    }

    /** 入口顺序未变化时就地更新标签和点击回调，保留磁贴、滚动位置与翻页动画。 */
    public void updateEntries(List<Entry> entries) {
        if (entries.size() != tiles.size()) throw new IllegalArgumentException("Entry count changed");
        for (int index = 0; index < entries.size(); index++) tiles.get(index).updateEntry(entries.get(index));
    }

    /** 第 {@code index} 个条目（与 {@link #setEntries} 的顺序一致）。 */
    public Button entryView(int index) { return tiles.get(index); }

    public int entryCount() { return tiles.size(); }

    public void setState(int index, State state) { tiles.get(index).setState(state); }

    public PagedTileGrid grid() { return grid; }

    public KeyboardPagerDots dots() { return dots; }

    /** 状态对应的 stateDescription；{@link State#NONE} 返回 {@code null}。 */
    public static String stateText(State state) {
        return switch (state) {
            case ON -> "已开启";
            case OFF -> "已关闭";
            case UNAVAILABLE -> "不可用";
            case NONE -> null;
        };
    }

    /** 面板里的一个条目：节点 text 是标签，自绘图标与标签，不画底色。 */
    static final class Tile extends Button {
        private static final float ICON_AREA_DP = 30f;
        private static final float ICON_DP = 24f;
        private static final float GLYPH_BOX_DP = 26f;
        private static final float GLYPH_RADIUS_DP = 5f;
        private static final float GLYPH_STROKE_DP = 1.8f;
        private static final float GLYPH_SP = 14f;
        private static final float LABEL_SP = 12.5f;
        private static final float LABEL_GAP_DP = 7f;
        private static final float BADGE_DP = 13f;
        private static final float BADGE_RING_DP = 2f;
        private static final float BADGE_CHECK_DP = 9f;
        private static final int PRESSED_ALPHA = 140;

        private final FunctionPanelView panel;
        private Entry entry;
        private final Paint iconPaint = new Paint(Paint.ANTI_ALIAS_FLAG);
        private final Paint boxPaint = new Paint(Paint.ANTI_ALIAS_FLAG);
        private final TextPaint textPaint = new TextPaint(Paint.ANTI_ALIAS_FLAG);
        private final Paint badgePaint = new Paint(Paint.ANTI_ALIAS_FLAG);
        private final RectF rect = new RectF();
        private String fittedLabel;
        private String fittedLabelSource;
        private float fittedLabelWidth = Float.NaN;
        private float fittedLabelSize = Float.NaN;
        private boolean fittedLabelBold;
        private State state = State.NONE;

        Tile(Context context, FunctionPanelView panel, Entry entry) {
            super(context);
            this.panel = panel;
            ViewPolicy.setAllCapsFalse(this);
            ViewPolicy.clearChrome(this);
            textPaint.setTextAlign(Paint.Align.CENTER);
            boxPaint.setStyle(Paint.Style.STROKE);
            updateEntry(entry);
        }

        void updateEntry(Entry value) {
            entry = value;
            if (!TextUtils.equals(getText(), entry.label)) setText(entry.label);
            String description = entry.description == null || entry.description.isEmpty()
                ? entry.label : entry.description;
            if (!TextUtils.equals(getContentDescription(), description)) setContentDescription(description);
            if (entry.action != null) ViewPolicy.bindClick(this, () -> this.entry.action.run());
            else setOnClickListener(null);
            if (entry.longPress != null) {
                setOnLongClickListener(view -> {
                    this.entry.longPress.run();
                    return true;
                });
            } else setOnLongClickListener(null);
            invalidate();
        }

        void setState(State value) {
            state = value;
            ViewPolicy.setSelected(this, value == State.ON);
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) setStateDescription(stateText(value));
            invalidate();
        }

        /** 条目不画底色；键盘的整树样式通道会给每个 Button 套键帽，这里一律挡掉，免得开启态被画成实心色块。 */
        @Override public void setBackground(Drawable background) {
            super.setBackground(null);
        }

        @Override public void setPressed(boolean pressed) {
            boolean changed = pressed != isPressed();
            super.setPressed(pressed);
            if (changed) invalidate();
        }

        /** 条目比原尺寸矮时的缩放比例：网格按面板高度缩小磁贴（{@link PagedTileGrid#pickerGrid}），条目里的各个尺寸乘同一个比例。 */
        private float scale() {
            float full = DimensionPolicy.floatPixels(getContext(), ITEM_HEIGHT_DP);
            return getHeight() <= 0 || full <= 0 ? 1f : BoundsPolicy.bounded(getHeight() / full, 0f, 1f);
        }

        /** 缩放后的 dp 换成像素。 */
        private float scaled(float dp, float scale) {
            return DimensionPolicy.floatPixels(getContext(), dp) * scale;
        }

        private void configureLabelPaint(float scale) {
            ViewPolicy.setTextSize(textPaint, KeyboardTypographyPolicy.keySp(getContext(), LABEL_SP) * scale);
            ViewPolicy.setTypeface(textPaint, Typeface.DEFAULT);
        }

        @Override protected void onDraw(Canvas canvas) {
            boolean on = state == State.ON;
            int color = on ? panel.accent : panel.foreground;
            if (isPressed()) color = ColorPolicy.withAlpha(color,
                Color.alpha(color) * PRESSED_ALPHA / 255);
            float scale = scale();
            configureLabelPaint(scale);
            Paint.FontMetrics label = textPaint.getFontMetrics();
            float labelHeight = label.descent - label.ascent;
            float area = scaled(ICON_AREA_DP, scale);
            float labelGap = scaled(LABEL_GAP_DP, scale);
            float total = area + labelGap + labelHeight;
            float top = BoundsPolicy.nonNegative((getHeight() - total) / 2f);
            float centerX = getWidth() / 2f;
            float iconCenterY = top + area / 2f;
            float iconRight;
            float iconBottom;
            if (entry.glyph != null) {
                float box = scaled(GLYPH_BOX_DP, scale);
                rect.set(centerX - box / 2f, iconCenterY - box / 2f, centerX + box / 2f,
                    iconCenterY + box / 2f);
                boxPaint.setColor(color);
                float stroke = scaled(GLYPH_STROKE_DP, scale);
                boxPaint.setStrokeWidth(stroke);
                rect.inset(stroke / 2f, stroke / 2f);
                float radius = scaled(GLYPH_RADIUS_DP, scale);
                canvas.drawRoundRect(rect, radius, radius, boxPaint);
                Paint glyph = textPaint;
                ViewPolicy.setTextSize(glyph, KeyboardTypographyPolicy.keySp(getContext(), GLYPH_SP) * scale);
                ViewPolicy.setTypeface(glyph, Typeface.DEFAULT_BOLD);
                glyph.setColor(color);
                Paint.FontMetrics metrics = glyph.getFontMetrics();
                canvas.drawText(entry.glyph, centerX,
                    iconCenterY - (metrics.ascent + metrics.descent) / 2f, glyph);
                iconRight = centerX + box / 2f;
                iconBottom = iconCenterY + box / 2f;
                configureLabelPaint(scale);
            } else {
                float size = scaled(ICON_DP, scale);
                if (entry.icon != null) {
                    KeyboardIconPaths.draw(canvas, iconPaint, entry.icon, centerX - size / 2f,
                        iconCenterY - size / 2f, size, color);
                }
                iconRight = centerX + size / 2f;
                iconBottom = iconCenterY + size / 2f;
            }
            if (on) drawBadge(canvas, iconRight, iconBottom, scale);
            textPaint.setColor(color);
            float baseline = top + area + labelGap - label.ascent;
            canvas.drawText(fit(entry.label, getWidth()), centerX, baseline, textPaint);
        }

        private void drawBadge(Canvas canvas, float iconRight, float iconBottom, float scale) {
            float badge = scaled(BADGE_DP, scale);
            float cx = iconRight;
            float cy = iconBottom;
            badgePaint.setStyle(Paint.Style.FILL);
            badgePaint.setColor(panel.panelBackground);
            canvas.drawCircle(cx, cy, badge / 2f + scaled(BADGE_RING_DP, scale), badgePaint);
            badgePaint.setColor(panel.accent);
            canvas.drawCircle(cx, cy, badge / 2f, badgePaint);
            float check = scaled(BADGE_CHECK_DP, scale);
            KeyboardIconPaths.draw(canvas, iconPaint, KeyboardIconPaths.Icon.CHECK,
                cx - check / 2f, cy - check / 2f, check, panel.panelBackground);
        }

        /** 放不下时以「…」结尾截断，不悄悄丢掉末尾的字。 */
        private String fit(String value, float width) {
            String source = TextPolicy.emptyIfNull(value);
            float boundedWidth = BoundsPolicy.nonNegative(width);
            boolean bold = textPaint.getTypeface() == Typeface.DEFAULT_BOLD;
            float size = textPaint.getTextSize();
            if (fittedLabel == null || !source.equals(fittedLabelSource)
                || fittedLabelWidth != boundedWidth || fittedLabelSize != size
                || fittedLabelBold != bold) {
                fittedLabel = TextUtils.ellipsize(source, textPaint, boundedWidth,
                    TextUtils.TruncateAt.END).toString();
                fittedLabelSource = source;
                fittedLabelWidth = boundedWidth;
                fittedLabelSize = size;
                fittedLabelBold = bold;
            }
            return fittedLabel;
        }

    }

    @Override protected void onDetachedFromWindow() {
        super.onDetachedFromWindow();
        for (View tile : tiles) tile.setPressed(false);
    }
}
