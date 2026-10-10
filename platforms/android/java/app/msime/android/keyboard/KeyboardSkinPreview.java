package app.msime.android;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.RectF;
import android.graphics.Path;
import android.graphics.Typeface;
import android.view.View;
/**
 * Small deterministic keyboard miniature used by the in-keyboard skin picker.
 *
 * <p>{@link #drawTile} 是新皮肤面板瓷砖用的版本：按设计 MiniKb 画一整副缩小的 26 键键盘（工具栏、四行带提示字的键、指示条）。{@link #drawPreview} 保留给仍需要大号带字缩略图的地方。
 */
public final class KeyboardSkinPreview extends View {
    private interface MiniDrawState {
        Paint paint();
        Paint text();
        RectF rect();
        Path chevron();
    }

    static final class TileDrawState implements MiniDrawState {
        final KeyboardSkinBackgroundDrawable background;
        final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
        final Paint text = new Paint(Paint.ANTI_ALIAS_FLAG);
        final RectF rect = new RectF();
        final Path clip = new Path();
        final Path chevron = new Path();

        TileDrawState(KeyboardSkin skin, float density) {
            background = new KeyboardSkinBackgroundDrawable(skin, density);
        }

        @Override public Paint paint() { return paint; }
        @Override public Paint text() { return text; }
        @Override public RectF rect() { return rect; }
        @Override public Path chevron() { return chevron; }
    }

    static final class SplitDrawState implements MiniDrawState {
        final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
        final Paint text = new Paint(Paint.ANTI_ALIAS_FLAG);
        final RectF rect = new RectF();
        final Path clip = new Path();
        final Path half = new Path();
        final int returnBackground;
        final int returnForeground;
        final int keyForeground;
        final int accent;

        SplitDrawState() {
            KeyboardSkin system = KeyboardSkin.system(false);
            returnBackground = Color.parseColor(system.returnBackground());
            returnForeground = Color.parseColor(system.returnForeground());
            keyForeground = Color.parseColor(system.keyForeground());
            accent = Color.parseColor(system.accent());
        }

        @Override public Paint paint() { return paint; }
        @Override public Paint text() { return text; }
        @Override public RectF rect() { return rect; }
        @Override public Path chevron() { return half; }
    }

    private static final String[][] PREVIEW_ROWS = {
        {"Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P"},
        {"A", "S", "D", "F", "G", "H", "J", "K", "L"},
        {"⇧", "Z", "X", "C", "V", "B", "N", "M", "⌫"},
        {"123", "空格", "↵"}
    };
    private KeyboardSkin skin;
    private final RectF previewBounds = new RectF();
    private final Paint previewText = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final RectF previewKey = new RectF();
    private PreviewDrawState previewDrawState;

    private static final class PreviewDrawState {
        final KeyboardSkinBackgroundDrawable background;
        final KeyboardSkinKeyDrawable[][] keys;

        PreviewDrawState(KeyboardSkin skin, float density) {
            background = new KeyboardSkinBackgroundDrawable(skin, density);
            keys = new KeyboardSkinKeyDrawable[PREVIEW_ROWS.length][];
            int keyFill = Color.parseColor(skin.keyBackground());
            int actionFill = Color.parseColor(skin.actionBackground());
            for (int rowIndex = 0; rowIndex < PREVIEW_ROWS.length; rowIndex++) {
                String[] row = PREVIEW_ROWS[rowIndex];
                keys[rowIndex] = new KeyboardSkinKeyDrawable[row.length];
                for (int index = 0; index < row.length; index++) {
                    boolean action = rowIndex == PREVIEW_ROWS.length - 1
                        || index == 0 && rowIndex == 2;
                    keys[rowIndex][index] = new KeyboardSkinKeyDrawable(skin,
                        action ? actionFill : keyFill, action, density);
                }
            }
        }
    }

    public KeyboardSkinPreview(Context context, KeyboardSkin skin) {
        super(context);
        this.skin = skin;
        previewDrawState = new PreviewDrawState(skin, KeyboardGeometry.density(context));
        setWillNotDraw(false);
    }

    public void setSkin(KeyboardSkin value) {
        skin = value;
        previewDrawState = new PreviewDrawState(value, KeyboardGeometry.density(getContext()));
        invalidate();
    }

    @Override protected void onDraw(Canvas canvas) {
        super.onDraw(canvas);
        previewBounds.set(0, 0, getWidth(), getHeight());
        drawPreview(canvas, previewBounds, skin, KeyboardGeometry.density(getContext()),
            previewText, previewKey, previewDrawState);
    }

    public static void drawPreview(Canvas canvas, RectF bounds, KeyboardSkin skin,
                                   float density) {
        drawPreview(canvas, bounds, skin, density, null, null);
    }

    private static void drawPreview(Canvas canvas, RectF bounds, KeyboardSkin skin,
                                    float density, Paint reusableText, RectF reusableKey) {
        drawPreview(canvas, bounds, skin, density, reusableText, reusableKey, null);
    }

    private static void drawPreview(Canvas canvas, RectF bounds, KeyboardSkin skin,
                                    float density, Paint reusableText, RectF reusableKey,
                                    PreviewDrawState state) {
        if (bounds.width() <= 0 || bounds.height() <= 0) return;
        KeyboardSkinBackgroundDrawable background = state == null
            ? new KeyboardSkinBackgroundDrawable(skin, density) : state.background;
        background.setBounds(Math.round(bounds.left), Math.round(bounds.top),
            Math.round(bounds.right), Math.round(bounds.bottom));
        background.draw(canvas);
        String[][] rows = PREVIEW_ROWS;
        float gap = BoundsPolicy.bounded(
            KeyboardGeometry.shorterSide(bounds.width(), bounds.height()) * .035f,
            1f, Float.MAX_VALUE);
        float rowHeight = (bounds.height() - gap * 3) / rows.length;
        Paint text = reusableText == null ? new Paint(Paint.ANTI_ALIAS_FLAG) : reusableText;
        text.setTextAlign(Paint.Align.CENTER);
        ViewPolicy.setTypeface(text, skin.monospaced() ? Typeface.MONOSPACE : Typeface.DEFAULT);
        ViewPolicy.setTextSize(text, KeyboardGeometry.bounded(rowHeight * .42f, 7f,
            KeyboardGeometry.floatPixels(14f, density)));
        int keyFill = Color.parseColor(skin.keyBackground());
        int actionFill = Color.parseColor(skin.actionBackground());
        int keyText = Color.parseColor(skin.keyForeground());
        int actionText = Color.parseColor(skin.actionForeground());
        RectF key = reusableKey == null ? new RectF() : reusableKey;
        for (int rowIndex = 0; rowIndex < rows.length; rowIndex++) {
            String[] row = rows[rowIndex];
            float rowInset = rowIndex == 1 ? bounds.width() * .04f : 0;
            float keyWidth = (bounds.width() - rowInset * 2 - gap * (row.length - 1))
                / row.length;
            for (int index = 0; index < row.length; index++) {
                float left = bounds.left + rowInset + index * (keyWidth + gap);
                float top = bounds.top + rowIndex * (rowHeight + gap);
                key.set(left, top, left + keyWidth, top + rowHeight);
                boolean action = rowIndex == rows.length - 1 || index == 0 && rowIndex == 2;
                KeyboardSkinKeyDrawable drawable = state == null ? new KeyboardSkinKeyDrawable(
                    skin, action ? actionFill : keyFill, action, density)
                    : state.keys[rowIndex][index];
                drawable.setBounds(Math.round(key.left), Math.round(key.top),
                    Math.round(key.right), Math.round(key.bottom));
                drawable.draw(canvas);
                text.setColor(action ? actionText : keyText);
                Paint.FontMetrics metrics = text.getFontMetrics();
                float baseline = key.centerY() - (metrics.ascent + metrics.descent) / 2;
                canvas.drawText(row[index], key.centerX(), baseline, text);
            }
        }
    }

    /** 设计 MiniKb 的虚拟画布宽度（px）；缩略图按瓷砖宽度等比缩放它。 */
    public static final float MINI_WIDTH = 390f;
    /** 设计 MiniKb 带工具栏时的虚拟画布高度：工具栏 50、间距 8、四行 43、三个 11 的行距，再加底部指示条区域。 */
    public static final float MINI_HEIGHT = 292f;

    /**
     * 瓷砖缩略图：设计的 MiniKb，即一整副缩小的 26 键键盘。
     *
     * 在 390×292 的虚拟画布上画工具栏（品牌块、五个图标、收起箭头）、四行键（字母键用键帽色并带右上角提示字，Shift、退格、123、中用功能键色，回车用回车色）和底部指示条，再按瓷砖宽度等比缩放。原先只画三行无字的小方块，和设计的瓷砖对不上。
     */
    public static void drawTile(Canvas canvas, RectF bounds, float radius, KeyboardSkin skin,
                                float density) {
        drawTile(canvas, bounds, radius, skin, density, new TileDrawState(skin, density));
    }

    static void drawTile(Canvas canvas, RectF bounds, float radius, KeyboardSkin skin,
                         float density, TileDrawState state) {
        if (bounds.width() <= 0 || bounds.height() <= 0) return;
        int saved = canvas.save();
        clipRound(canvas, bounds, radius, state.clip);
        state.background.setBounds(Math.round(bounds.left), Math.round(bounds.top),
            Math.round(bounds.right), Math.round(bounds.bottom));
        state.background.draw(canvas);
        double opacity = skin.designed() ? skin.keyOpacity() : 1d;
        drawMiniKeyboard(canvas, bounds,
            ColorPolicy.withAlpha(Color.parseColor(skin.keyBackground()), opacity),
            ColorPolicy.withAlpha(Color.parseColor(skin.functionBackground()), opacity),
            Color.parseColor(skin.returnBackground()), Color.parseColor(skin.returnForeground()),
            Color.parseColor(skin.keyForeground()), Color.parseColor(skin.secondary()),
            Color.parseColor(skin.accent()), state);
        canvas.restoreToCount(saved);
    }

    /** 「跟随系统」瓷砖：135° 对角线把浅色与深色对半分，上面是设计给它的白色半透明键帽（rgba(255,255,255,.85)）。 */
    public static void drawSplit(Canvas canvas, RectF bounds, float radius, int light, int dark) {
        drawSplit(canvas, bounds, radius, light, dark, new SplitDrawState());
    }

    static void drawSplit(Canvas canvas, RectF bounds, float radius, int light, int dark,
                          SplitDrawState state) {
        if (bounds.width() <= 0 || bounds.height() <= 0) return;
        int saved = canvas.save();
        clipRound(canvas, bounds, radius, state.clip);
        Paint paint = state.paint;
        paint.setColor(light);
        canvas.drawRect(bounds, paint);
        Path half = state.half;
        half.reset();
        half.moveTo(bounds.right, bounds.top);
        half.lineTo(bounds.right, bounds.bottom);
        half.lineTo(bounds.left, bounds.bottom);
        half.close();
        paint.setColor(dark);
        canvas.drawPath(half, paint);
        int keys = ColorPolicy.withAlpha(Color.WHITE, 217);
        drawMiniKeyboard(canvas, bounds, keys, keys, state.returnBackground,
            state.returnForeground, state.keyForeground, ColorPolicy.withAlpha(Color.BLACK, 115), state.accent,
            state);
        canvas.restoreToCount(saved);
    }

    private static void clipRound(Canvas canvas, RectF bounds, float radius) {
        clipRound(canvas, bounds, radius, new Path());
    }

    private static void clipRound(Canvas canvas, RectF bounds, float radius, Path clip) {
        clip.reset();
        clip.addRoundRect(bounds, radius, radius, android.graphics.Path.Direction.CW);
        canvas.clipPath(clip);
    }

    /** MiniKb 字母键右上角的提示字，与设计的 HINT 表一一对应。 */
    private static final String LETTER_HINTS = "qwertyuiopasdfghjklzxcvbnm";
    private static final String[] HINTS = {"1", "2", "3", "4", "5", "6", "7", "8", "9", "0",
        "@", "#", "¥", "%", "&", "*", "(", ")", "\"", "~", "…", "、", "?", "!", "-", "/"};
    private static final String[][] MINI_ROWS = {
        {"q", "w", "e", "r", "t", "y", "u", "i", "o", "p"},
        {"a", "s", "d", "f", "g", "h", "j", "k", "l"},
        {"⇧", "z", "x", "c", "v", "b", "n", "m", "⌫"},
        {"123", "，", "", "。", "中", "↵"},
    };
    private static final float[][] MINI_WEIGHTS = {
        null, null, {1.4f, 1, 1, 1, 1, 1, 1, 1, 1.4f}, {1.25f, 1, 4, 1, 1.05f, 1.9f},
    };

    /**
     * 在 bounds 上按 MINI_WIDTH 等比缩放画 MiniKb 的工具栏、键区和指示条。尺寸都是设计的虚拟像素：内边距 3、工具栏 50、工具栏与键区间距 8、行高 43、行距 11、键距 6、键角 5、键帽底部 1 px 阴影。
     */
    private static void drawMiniKeyboard(Canvas canvas, RectF bounds, int key, int function,
                                         int returnFill, int returnInk, int ink, int sub, int accent) {
        drawMiniKeyboard(canvas, bounds, key, function, returnFill, returnInk, ink, sub, accent,
            null);
    }

    private static void drawMiniKeyboard(Canvas canvas, RectF bounds, int key, int function,
                                         int returnFill, int returnInk, int ink, int sub, int accent,
                                         MiniDrawState state) {
        float sc = bounds.width() / MINI_WIDTH;
        int saved = canvas.save();
        canvas.translate(bounds.left, bounds.top);
        canvas.scale(sc, sc);
        Paint paint = state == null ? new Paint(Paint.ANTI_ALIAS_FLAG) : state.paint();
        RectF rect = state == null ? new RectF() : state.rect();
        // 工具栏：品牌块、五个图标、收起箭头，七列等分。
        float column = (MINI_WIDTH - 6f) / 7f;
        for (int index = 0; index < 7; index++) {
            float cx = 3f + column * (index + .5f);
            if (index == 0) {
                paint.setStyle(Paint.Style.FILL);
                paint.setColor(accent);
                rect.set(cx - 13f, 12f, cx + 13f, 38f);
                canvas.drawRoundRect(rect, 8f, 8f, paint);
            } else if (index == 6) {
                paint.setStyle(Paint.Style.STROKE);
                paint.setStrokeWidth(1.8f);
                paint.setColor(ink);
                Path chevron = state == null ? new Path() : state.chevron();
                chevron.reset();
                chevron.moveTo(cx - 6f, 22f);
                chevron.lineTo(cx, 28f);
                chevron.lineTo(cx + 6f, 22f);
                canvas.drawPath(chevron, paint);
            } else {
                paint.setStyle(Paint.Style.STROKE);
                paint.setStrokeWidth(1.7f);
                paint.setColor(ink);
                rect.set(cx - 9f, 16f, cx + 9f, 34f);
                canvas.drawRoundRect(rect, 4f, 4f, paint);
            }
        }
        paint.setStyle(Paint.Style.FILL);
        Paint text = state == null ? new Paint(Paint.ANTI_ALIAS_FLAG) : state.text();
        float top = 50f + 8f;
        for (int row = 0; row < MINI_ROWS.length; row++) {
            String[] line = MINI_ROWS[row];
            float[] weights = MINI_WEIGHTS[row];
            float inset = row == 1 ? (MINI_WIDTH - 6f) * .05f : 0f;
            float width = MINI_WIDTH - 6f - inset * 2f - 6f * (line.length - 1);
            float total = 0;
            for (int index = 0; index < line.length; index++) total += weights == null ? 1f : weights[index];
            float x = 3f + inset;
            for (int index = 0; index < line.length; index++) {
                float keyWidth = width * (weights == null ? 1f : weights[index]) / total;
                String face = line[index];
                boolean letter = face.length() == 1 && LETTER_HINTS.contains(face);
                boolean enter = "↵".equals(face);
                boolean punctuation = "，".equals(face) || "。".equals(face);
                boolean special = !letter && !enter && !punctuation && !face.isEmpty();
                rect.set(x, top, x + keyWidth, top + 43f);
                paint.setColor(0x47000000);
                canvas.drawRoundRect(rect.left, rect.top + 1f, rect.right, rect.bottom + 1f, 5f, 5f, paint);
                paint.setColor(enter ? returnFill : special ? function : key);
                canvas.drawRoundRect(rect, 5f, 5f, paint);
                if (!face.isEmpty()) {
                    text.setTextAlign(Paint.Align.CENTER);
                    text.setColor(enter ? returnInk : ink);
                    ViewPolicy.setTextSize(text, letter || punctuation ? 22f : 15f);
                    text.setFakeBoldText(special);
                    Paint.FontMetrics metrics = text.getFontMetrics();
                    canvas.drawText(face, rect.centerX(), rect.centerY() - (metrics.ascent + metrics.descent) / 2f, text);
                }
                if (letter) {
                    text.setTextAlign(Paint.Align.RIGHT);
                    text.setColor(sub);
                    ViewPolicy.setTextSize(text, 10f);
                    text.setFakeBoldText(false);
                    canvas.drawText(HINTS[LETTER_HINTS.indexOf(face)], rect.right - 5f, rect.top + 13f, text);
                }
                x += keyWidth + 6f;
            }
            top += 43f + 11f;
        }
        // 底部指示条：134×5，键字色 85%，居中在键区下方的余量里。
        float keysBottom = top - 11f;
        float barTop = keysBottom + (MINI_HEIGHT - keysBottom) / 2f - 2.5f;
        paint.setColor(ColorPolicy.withAlpha(ink, .85));
        rect.set(MINI_WIDTH / 2f - 67f, barTop, MINI_WIDTH / 2f + 67f, barTop + 5f);
        canvas.drawRoundRect(rect, 3f, 3f, paint);
        canvas.restoreToCount(saved);
    }

    /** 瓷砖缩略图每行的键数。 */
    public static int[] tileRowCounts() { return new int[] {5, 5, 3}; }
}
