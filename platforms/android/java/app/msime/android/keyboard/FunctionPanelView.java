package app.msime.android;

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
import android.util.TypedValue;
import android.view.Gravity;
import android.view.View;
import android.widget.Button;
import android.widget.LinearLayout;
import java.util.ArrayList;
import java.util.List;

/**
 * 功能面板（设计的「菜单」）：横向分页、每页 4 列 × 2 行的条目，下方页点。
 *
 * <p>条目没有底色：30 dp 的图标区加 12.5 sp 的标签，间距 7 dp，条目高 52 dp。字形条目（全、，、≈、繁）画 26×26、圆角 5、1.8 dp 描边的方框加 14 sp 粗体字。开启状态用 accent、标签加粗，图标右下角加 13 dp 的 accent ✓ 角标（2 dp 的 kbBg 圆环）。
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

    private final PagedTileGrid grid;
    private final KeyboardPagerDots dots;
    private final List<Tile> tiles = new ArrayList<>();
    private int foreground = Color.BLACK;
    private int accent = Color.BLUE;
    private int panelBackground = Color.WHITE;

    public FunctionPanelView(Context context) {
        super(context);
        setOrientation(VERTICAL);
        setContentDescription("更多工具");
        float density = getResources().getDisplayMetrics().density;
        setPadding(0, Math.round(10 * density), 0, Math.round(4 * density));
        grid = new PagedTileGrid(context);
        grid.setGrid(4, 2);
        grid.setSpacing(ITEM_HEIGHT_DP, 16f, 4f, 4f);
        dots = new KeyboardPagerDots(context);
        grid.setOnPageChangeListener((page, count) -> dots.setActive(page, true));
        LinearLayout.LayoutParams gridParams = new LinearLayout.LayoutParams(
            LayoutParams.MATCH_PARENT, 0, 1f);
        addView(grid, gridParams);
        LinearLayout.LayoutParams dotParams = new LinearLayout.LayoutParams(
            LayoutParams.WRAP_CONTENT, LayoutParams.WRAP_CONTENT);
        dotParams.gravity = Gravity.CENTER_HORIZONTAL;
        dotParams.topMargin = Math.round(8 * density);
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

    /** 换掉全部条目；状态都重置为 {@link State#NONE}。 */
    public void setEntries(List<Entry> entries) {
        grid.removeAllViews();
        tiles.clear();
        for (Entry entry : entries) {
            Tile tile = new Tile(getContext(), this, entry);
            tiles.add(tile);
            grid.addView(tile);
        }
        dots.setCount(grid.pageCount());
        dots.setActive(grid.page(), false);
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
        private final Entry entry;
        private final Paint iconPaint = new Paint(Paint.ANTI_ALIAS_FLAG);
        private final Paint boxPaint = new Paint(Paint.ANTI_ALIAS_FLAG);
        private final TextPaint textPaint = new TextPaint(Paint.ANTI_ALIAS_FLAG);
        private final Paint badgePaint = new Paint(Paint.ANTI_ALIAS_FLAG);
        private final RectF rect = new RectF();
        private State state = State.NONE;

        Tile(Context context, FunctionPanelView panel, Entry entry) {
            super(context);
            this.panel = panel;
            this.entry = entry;
            setText(entry.label);
            setAllCaps(false);
            setBackground(null);
            setPadding(0, 0, 0, 0);
            setMinWidth(0);
            setMinimumWidth(0);
            setMinHeight(0);
            setMinimumHeight(0);
            setContentDescription(entry.description == null || entry.description.isEmpty()
                ? entry.label : entry.description);
            textPaint.setTextAlign(Paint.Align.CENTER);
            boxPaint.setStyle(Paint.Style.STROKE);
            if (entry.action != null) setOnClickListener(view -> entry.action.run());
            if (entry.longPress != null) {
                setOnLongClickListener(view -> {
                    entry.longPress.run();
                    return true;
                });
            }
        }

        void setState(State value) {
            state = value;
            setSelected(value == State.ON);
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

        @Override protected void onDraw(Canvas canvas) {
            float density = getResources().getDisplayMetrics().density;
            boolean on = state == State.ON;
            int color = on ? panel.accent : panel.foreground;
            if (isPressed()) color = Color.argb(Color.alpha(color) * PRESSED_ALPHA / 255,
                Color.red(color), Color.green(color), Color.blue(color));
            textPaint.setTextSize(sp(LABEL_SP));
            textPaint.setTypeface(on ? Typeface.DEFAULT_BOLD : Typeface.DEFAULT);
            Paint.FontMetrics label = textPaint.getFontMetrics();
            float labelHeight = label.descent - label.ascent;
            float area = ICON_AREA_DP * density;
            float total = area + LABEL_GAP_DP * density + labelHeight;
            float top = Math.max(0f, (getHeight() - total) / 2f);
            float centerX = getWidth() / 2f;
            float iconCenterY = top + area / 2f;
            float iconRight;
            float iconBottom;
            if (entry.glyph != null) {
                float box = GLYPH_BOX_DP * density;
                rect.set(centerX - box / 2f, iconCenterY - box / 2f, centerX + box / 2f,
                    iconCenterY + box / 2f);
                boxPaint.setColor(color);
                boxPaint.setStrokeWidth(GLYPH_STROKE_DP * density);
                float inset = GLYPH_STROKE_DP * density / 2f;
                rect.inset(inset, inset);
                float radius = GLYPH_RADIUS_DP * density;
                canvas.drawRoundRect(rect, radius, radius, boxPaint);
                Paint glyph = textPaint;
                glyph.setTextSize(sp(GLYPH_SP));
                glyph.setTypeface(Typeface.DEFAULT_BOLD);
                glyph.setColor(color);
                Paint.FontMetrics metrics = glyph.getFontMetrics();
                canvas.drawText(entry.glyph, centerX,
                    iconCenterY - (metrics.ascent + metrics.descent) / 2f, glyph);
                iconRight = centerX + box / 2f;
                iconBottom = iconCenterY + box / 2f;
                textPaint.setTextSize(sp(LABEL_SP));
                textPaint.setTypeface(on ? Typeface.DEFAULT_BOLD : Typeface.DEFAULT);
            } else {
                float size = ICON_DP * density;
                if (entry.icon != null) {
                    KeyboardIconPaths.draw(canvas, iconPaint, entry.icon, centerX - size / 2f,
                        iconCenterY - size / 2f, size, color);
                }
                iconRight = centerX + size / 2f;
                iconBottom = iconCenterY + size / 2f;
            }
            if (on) drawBadge(canvas, density, iconRight, iconBottom);
            textPaint.setColor(color);
            float baseline = top + area + LABEL_GAP_DP * density - label.ascent;
            canvas.drawText(fit(entry.label, getWidth()), centerX, baseline, textPaint);
        }

        private void drawBadge(Canvas canvas, float density, float iconRight, float iconBottom) {
            float badge = BADGE_DP * density;
            float cx = iconRight;
            float cy = iconBottom;
            badgePaint.setStyle(Paint.Style.FILL);
            badgePaint.setColor(panel.panelBackground);
            canvas.drawCircle(cx, cy, badge / 2f + BADGE_RING_DP * density, badgePaint);
            badgePaint.setColor(panel.accent);
            canvas.drawCircle(cx, cy, badge / 2f, badgePaint);
            float check = BADGE_CHECK_DP * density;
            KeyboardIconPaths.draw(canvas, iconPaint, KeyboardIconPaths.Icon.CHECK,
                cx - check / 2f, cy - check / 2f, check, panel.panelBackground);
        }

        /** 放不下时以「…」结尾截断，不悄悄丢掉末尾的字。 */
        private String fit(String value, float width) {
            return TextUtils.ellipsize(value == null ? "" : value, textPaint, Math.max(0f, width),
                TextUtils.TruncateAt.END).toString();
        }

        private float sp(float value) {
            return TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_SP, value,
                getResources().getDisplayMetrics());
        }
    }

    @Override protected void onDetachedFromWindow() {
        super.onDetachedFromWindow();
        for (View tile : tiles) tile.setPressed(false);
    }
}
