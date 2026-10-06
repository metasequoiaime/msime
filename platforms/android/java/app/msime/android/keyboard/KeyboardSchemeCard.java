package app.msime.android;

import android.content.Context;
import android.graphics.Color;
import android.graphics.Path;
import android.graphics.drawable.GradientDrawable;
import android.util.TypedValue;
import android.view.Gravity;
import android.view.View;
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import android.widget.TextView;

/**
 * 输入方案选择器里的一张卡片：字形、角标、标题，选中的右上角有个对勾。
 *
 * <p>Built from the iOS picker, `KeyboardSchemePickerView.makeCard`, down to the measurements: a
 * 27dp bordered glyph, a 16x12 badge overhanging its bottom-right corner, a 12dp check overhanging
 * the top-right, and a 12sp title that shrinks rather than wraps.
 *
 * <p>Every card takes the skin's accent, as the master does. The per-family colours this replaced
 * -- 双拼蓝, 五笔棕, 日语粉, 手写青, 回复橙 -- were fixed system colours, so under a dark or custom skin
 * they were not grouping anything, only sitting apart from the keys and the candidate strip.
 *
 * <p>新设计的网格瓷砖样式走 {@link #paintTile}：没有卡片底色，未选中的字形、角标和标题用 kbFg，选中的用 accent、标题加粗，对勾挪到字形右下角（替换角标的位置）。旧的 {@link #paint} 保留，调用方切换前行为不变。
 */
public final class KeyboardSchemeCard extends FrameLayout {
    private static final float CARD_RADIUS_DP = 13f;
    private static final float GLYPH_SIZE_DP = 27f;
    private static final float GLYPH_RADIUS_DP = 4f;
    private static final float GLYPH_BORDER_DP = 1.7f;
    private static final float BADGE_WIDTH_DP = 16f;
    private static final float BADGE_HEIGHT_DP = 12f;
    private static final float CHECK_SIZE_DP = 12f;
    /** The check clears the glyph's top edge by this much, and the badge its bottom edge. */
    private static final float OVERHANG_TOP_DP = 3f;
    private static final float OVERHANG_BOTTOM_DP = 3f;

    private final TextView glyph;
    private final TextView badge;
    private final TextView title;
    private final View check;
    private final FrameLayout.LayoutParams checkCornerParams;
    private final FrameLayout.LayoutParams checkBadgeParams;
    private boolean selected;

    public KeyboardSchemeCard(Context context, String glyphText, String badgeText,
            String titleText) {
        super(context);
        setClickable(true);
        setFocusable(true);

        glyph = new TextView(context);
        glyph.setText(glyphText);
        glyph.setGravity(Gravity.CENTER);
        // "EN" is two characters wide in a box sized for one, so it takes the smaller face.
        glyph.setTextSize(TypedValue.COMPLEX_UNIT_SP, glyphText.length() > 1 ? 15 : 20);
        glyph.setTypeface(glyph.getTypeface(), android.graphics.Typeface.BOLD);

        badge = new TextView(context);
        badge.setText(badgeText);
        badge.setGravity(Gravity.CENTER);
        badge.setTextSize(TypedValue.COMPLEX_UNIT_SP, 9);
        badge.setTypeface(badge.getTypeface(), android.graphics.Typeface.BOLD);

        check = new View(context);
        check.setVisibility(View.GONE);

        title = new TextView(context);
        title.setText(titleText);
        title.setGravity(Gravity.CENTER);
        title.setMaxLines(1);
        title.setTextSize(TypedValue.COMPLEX_UNIT_SP, 12);
        title.setEllipsize(android.text.TextUtils.TruncateAt.END);

        // 字形、角标和对勾挤在一小块里，彼此的位置只跟字形框有关，跟卡片宽度无关；先把它们装进一个
        // 固定大小的簇，再把这个簇居中，就不必在布局时知道卡片有多宽。
        FrameLayout cluster = new FrameLayout(context);
        int glyphSize = KeyboardGeometry.pixels(getContext(), GLYPH_SIZE_DP);
        int badgeWidth = KeyboardGeometry.pixels(getContext(), BADGE_WIDTH_DP);
        int badgeHeight = KeyboardGeometry.pixels(getContext(), BADGE_HEIGHT_DP);
        int checkSize = KeyboardGeometry.pixels(getContext(), CHECK_SIZE_DP);
        int top = KeyboardGeometry.pixels(getContext(), OVERHANG_TOP_DP);

        FrameLayout.LayoutParams glyphParams = new FrameLayout.LayoutParams(glyphSize, glyphSize);
        glyphParams.topMargin = top;
        cluster.addView(glyph, glyphParams);

        FrameLayout.LayoutParams badgeParams =
            new FrameLayout.LayoutParams(badgeWidth, badgeHeight);
        badgeParams.leftMargin = glyphSize + KeyboardGeometry.pixels(getContext(), 4) - badgeWidth;
        badgeParams.topMargin = top + glyphSize + KeyboardGeometry.pixels(getContext(), OVERHANG_BOTTOM_DP) - badgeHeight;
        cluster.addView(badge, badgeParams);

        FrameLayout.LayoutParams checkParams = new FrameLayout.LayoutParams(checkSize, checkSize);
        checkParams.leftMargin = glyphSize + KeyboardGeometry.pixels(getContext(), 1);
        checkCornerParams = checkParams;
        checkBadgeParams = new FrameLayout.LayoutParams(checkSize, checkSize);
        checkBadgeParams.leftMargin = glyphSize + KeyboardGeometry.pixels(getContext(), 4) - checkSize;
        checkBadgeParams.topMargin = top + glyphSize + KeyboardGeometry.pixels(getContext(), OVERHANG_BOTTOM_DP) - checkSize;
        cluster.addView(check, checkParams);

        LinearLayout column = new LinearLayout(context);
        column.setOrientation(LinearLayout.VERTICAL);
        column.setGravity(Gravity.CENTER_HORIZONTAL);

        LinearLayout.LayoutParams clusterParams = new LinearLayout.LayoutParams(
            glyphSize + KeyboardGeometry.pixels(getContext(), 1) + checkSize, top + glyphSize + KeyboardGeometry.pixels(getContext(), OVERHANG_BOTTOM_DP));
        clusterParams.gravity = Gravity.CENTER_HORIZONTAL;
        // 8dp is measured to the glyph, and the cluster already carries the check's overhang.
        clusterParams.topMargin = KeyboardGeometry.pixels(getContext(), 8) - top;
        column.addView(cluster, clusterParams);

        LinearLayout.LayoutParams titleParams = new LinearLayout.LayoutParams(
            LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT);
        titleParams.topMargin = KeyboardGeometry.pixels(getContext(), 6) - KeyboardGeometry.pixels(getContext(), OVERHANG_BOTTOM_DP);
        titleParams.leftMargin = KeyboardGeometry.pixels(getContext(), 2);
        titleParams.rightMargin = KeyboardGeometry.pixels(getContext(), 2);
        column.addView(title, titleParams);

        FrameLayout.LayoutParams columnParams = new FrameLayout.LayoutParams(
            FrameLayout.LayoutParams.MATCH_PARENT, FrameLayout.LayoutParams.WRAP_CONTENT);
        columnParams.gravity = Gravity.CENTER_VERTICAL;
        addView(column, columnParams);

        for (View child : new View[] {glyph, badge, title, check, cluster, column}) {
            child.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
        }
    }

    /**
     * Paint the card for one skin.
     *
     * <p>Called after the keyboard-wide style pass, which walks every TextView and repaints it in
     * the key foreground; run before it, these colours would not survive.
     */
    public void paint(int accent, int keyBackground, boolean isSelected) {
        selected = isSelected;
        if (check.getLayoutParams() != checkCornerParams) check.setLayoutParams(checkCornerParams);
        badge.setVisibility(View.VISIBLE);
        title.setTypeface(android.graphics.Typeface.DEFAULT);
        // 选中与未选中的差别落在底色和这一档透明度上，不落在色相上。
        int face = isSelected ? accent : fade(accent, .78f);
        setBackground(rounded(isSelected ? fade(accent, .12f) : Color.TRANSPARENT,
            KeyboardGeometry.pixels(getContext(), CARD_RADIUS_DP)));
        glyph.setTextColor(face);
        glyph.setBackground(outlined(face, KeyboardGeometry.pixels(getContext(), GLYPH_RADIUS_DP), KeyboardGeometry.pixels(getContext(), GLYPH_BORDER_DP)));
        badge.setTextColor(face);
        // 角标和对勾都压在字形框的边线上，各自带一小块与面板同色的底，把边线断开。
        badge.setBackgroundColor(keyBackground);
        title.setTextColor(face);
        check.setBackground(checkMark(accent, keyBackground));
        check.setVisibility(isSelected ? View.VISIBLE : View.GONE);
    }

    /**
     * 按新设计的网格瓷砖样式上色：无底色，未选中用 {@code foreground}（kbFg），选中用 {@code accent} 且标题加粗，对勾落在字形右下角并替换角标。
     *
     * <p>和 {@link #paint} 一样要在键盘的样式通道之后调用。{@code panelBackground} 是面板底色（kbBg），用来断开字形框边线。
     */
    public void paintTile(int accent, int foreground, int panelBackground, boolean isSelected) {
        selected = isSelected;
        int face = isSelected ? accent : foreground;
        setBackground(null);
        glyph.setTextColor(face);
        glyph.setBackground(outlined(face, KeyboardGeometry.pixels(getContext(), GLYPH_RADIUS_DP), KeyboardGeometry.pixels(getContext(), GLYPH_BORDER_DP)));
        badge.setTextColor(face);
        badge.setBackgroundColor(panelBackground);
        badge.setVisibility(isSelected || badge.getText().length() == 0 ? View.INVISIBLE : View.VISIBLE);
        title.setTextColor(face);
        title.setTypeface(isSelected ? android.graphics.Typeface.DEFAULT_BOLD
            : android.graphics.Typeface.DEFAULT);
        if (check.getLayoutParams() != checkBadgeParams) check.setLayoutParams(checkBadgeParams);
        check.setBackground(checkMark(accent, panelBackground));
        check.setVisibility(isSelected ? View.VISIBLE : View.GONE);
    }

    public boolean isCardSelected() { return selected; }

    @Override public void setPressed(boolean pressed) {
        boolean changed = pressed != isPressed();
        super.setPressed(pressed);
        if (changed) KeyboardPressFeedback.update(this, pressed);
    }

    @Override protected void onDetachedFromWindow() {
        animate().cancel();
        KeyboardPressFeedback.reset(this);
        super.onDetachedFromWindow();
    }

    private static android.graphics.drawable.Drawable rounded(int color, int radius) {
        GradientDrawable shape = new GradientDrawable();
        shape.setShape(GradientDrawable.RECTANGLE);
        shape.setCornerRadius(radius);
        shape.setColor(color);
        return shape;
    }

    private static android.graphics.drawable.Drawable outlined(int color, int radius, int width) {
        GradientDrawable shape = new GradientDrawable();
        shape.setShape(GradientDrawable.RECTANGLE);
        shape.setCornerRadius(radius);
        shape.setColor(Color.TRANSPARENT);
        shape.setStroke(Math.max(1, width), color);
        return shape;
    }

    /** A filled disc carrying a tick, sized for the 12dp corner the master puts it in. */
    private android.graphics.drawable.Drawable checkMark(int accent, int keyBackground) {
        GradientDrawable disc = new GradientDrawable();
        disc.setShape(GradientDrawable.OVAL);
        disc.setColor(accent);
        // The disc sits on the glyph's border, so it carries the panel colour as its own ring.
        disc.setStroke(Math.max(1, KeyboardGeometry.pixels(getContext(), 1f)), keyBackground);
        return new android.graphics.drawable.LayerDrawable(
            new android.graphics.drawable.Drawable[] {disc, tick(keyBackground)});
    }

    private android.graphics.drawable.Drawable tick(int color) {
        android.graphics.drawable.ShapeDrawable mark =
            new android.graphics.drawable.ShapeDrawable(new CheckShape(color));
        mark.setIntrinsicWidth(KeyboardGeometry.pixels(getContext(), CHECK_SIZE_DP));
        mark.setIntrinsicHeight(KeyboardGeometry.pixels(getContext(), CHECK_SIZE_DP));
        return mark;
    }

    /** The tick itself: two strokes, drawn rather than shipped as one more density-split asset. */
    private static final class CheckShape extends android.graphics.drawable.shapes.Shape {
        private final android.graphics.Paint paint = new android.graphics.Paint(
            android.graphics.Paint.ANTI_ALIAS_FLAG);
        private final Path path = new Path();

        CheckShape(int color) {
            paint.setColor(color);
            paint.setStyle(android.graphics.Paint.Style.STROKE);
            paint.setStrokeCap(android.graphics.Paint.Cap.ROUND);
            paint.setStrokeJoin(android.graphics.Paint.Join.ROUND);
        }

        @Override public void draw(android.graphics.Canvas canvas, android.graphics.Paint ignored) {
            float width = getWidth();
            float height = getHeight();
            paint.setStrokeWidth(Math.max(1f, width * .16f));
            path.reset();
            path.moveTo(width * .28f, height * .52f);
            path.lineTo(width * .44f, height * .68f);
            path.lineTo(width * .73f, height * .34f);
            canvas.drawPath(path, paint);
        }
    }

    private static int fade(int color, float alpha) {
        return Color.argb(Math.round(Color.alpha(color) * alpha),
            Color.red(color), Color.green(color), Color.blue(color));
    }
}
