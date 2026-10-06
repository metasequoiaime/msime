package app.msime.android;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.RectF;
import android.graphics.Typeface;
import android.view.View;

/**
 * Small deterministic keyboard miniature used by the in-keyboard skin picker.
 *
 * <p>{@link #drawTile} 是新皮肤面板瓷砖用的版本：按设计 MiniKb 画一整副缩小的 26 键键盘（工具栏、四行带提示字的键、指示条）。{@link #drawPreview} 保留给仍需要大号带字缩略图的地方。
 */
public final class KeyboardSkinPreview extends View {
    private KeyboardSkin skin;

    public KeyboardSkinPreview(Context context, KeyboardSkin skin) {
        super(context);
        this.skin = skin;
        setWillNotDraw(false);
    }

    public void setSkin(KeyboardSkin value) {
        skin = value;
        invalidate();
    }

    @Override protected void onDraw(Canvas canvas) {
        super.onDraw(canvas);
        drawPreview(canvas, new RectF(0, 0, getWidth(), getHeight()), skin,
            getResources().getDisplayMetrics().density);
    }

    public static void drawPreview(Canvas canvas, RectF bounds, KeyboardSkin skin,
                                   float density) {
        if (bounds.width() <= 0 || bounds.height() <= 0) return;
        KeyboardSkinBackgroundDrawable background =
            new KeyboardSkinBackgroundDrawable(skin, density);
        background.setBounds(Math.round(bounds.left), Math.round(bounds.top),
            Math.round(bounds.right), Math.round(bounds.bottom));
        background.draw(canvas);
        String[][] rows = {
            {"Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P"},
            {"A", "S", "D", "F", "G", "H", "J", "K", "L"},
            {"⇧", "Z", "X", "C", "V", "B", "N", "M", "⌫"},
            {"123", "空格", "↵"}
        };
        float gap = Math.max(1, Math.min(bounds.width(), bounds.height()) * .035f);
        float rowHeight = (bounds.height() - gap * 3) / rows.length;
        Paint text = new Paint(Paint.ANTI_ALIAS_FLAG);
        text.setTextAlign(Paint.Align.CENTER);
        text.setTypeface(skin.monospaced() ? Typeface.MONOSPACE : Typeface.DEFAULT);
        text.setTextSize(KeyboardGeometry.bounded(rowHeight * .42f, 7f, 14f * density));
        int keyFill = Color.parseColor(skin.keyBackground());
        int actionFill = Color.parseColor(skin.actionBackground());
        int keyText = Color.parseColor(skin.keyForeground());
        int actionText = Color.parseColor(skin.actionForeground());
        RectF key = new RectF();
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
                int fill = action ? actionFill : keyFill;
                KeyboardSkinKeyDrawable drawable = new KeyboardSkinKeyDrawable(
                    skin, fill, action, density);
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
        if (bounds.width() <= 0 || bounds.height() <= 0) return;
        int saved = canvas.save();
        clipRound(canvas, bounds, radius);
        KeyboardSkinBackgroundDrawable background =
            new KeyboardSkinBackgroundDrawable(skin, density);
        background.setBounds(Math.round(bounds.left), Math.round(bounds.top),
            Math.round(bounds.right), Math.round(bounds.bottom));
        background.draw(canvas);
        double opacity = skin.designed() ? skin.keyOpacity() : 1d;
        drawMiniKeyboard(canvas, bounds,
            withOpacity(Color.parseColor(skin.keyBackground()), opacity),
            withOpacity(Color.parseColor(skin.functionBackground()), opacity),
            Color.parseColor(skin.returnBackground()), Color.parseColor(skin.returnForeground()),
            Color.parseColor(skin.keyForeground()), Color.parseColor(skin.secondary()),
            Color.parseColor(skin.accent()));
        canvas.restoreToCount(saved);
    }

    /** 「跟随系统」瓷砖：135° 对角线把浅色与深色对半分，上面是设计给它的白色半透明键帽（rgba(255,255,255,.85)）。 */
    public static void drawSplit(Canvas canvas, RectF bounds, float radius, int light, int dark) {
        if (bounds.width() <= 0 || bounds.height() <= 0) return;
        int saved = canvas.save();
        clipRound(canvas, bounds, radius);
        Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
        paint.setColor(light);
        canvas.drawRect(bounds, paint);
        android.graphics.Path half = new android.graphics.Path();
        half.moveTo(bounds.right, bounds.top);
        half.lineTo(bounds.right, bounds.bottom);
        half.lineTo(bounds.left, bounds.bottom);
        half.close();
        paint.setColor(dark);
        canvas.drawPath(half, paint);
        KeyboardSkin system = KeyboardSkin.system(false);
        int keys = Color.argb(217, 255, 255, 255);
        drawMiniKeyboard(canvas, bounds, keys, keys, Color.parseColor(system.returnBackground()),
            Color.parseColor(system.returnForeground()), Color.parseColor(system.keyForeground()),
            Color.argb(115, 0, 0, 0), Color.parseColor(system.accent()));
        canvas.restoreToCount(saved);
    }

    private static void clipRound(Canvas canvas, RectF bounds, float radius) {
        android.graphics.Path clip = new android.graphics.Path();
        clip.addRoundRect(bounds, radius, radius, android.graphics.Path.Direction.CW);
        canvas.clipPath(clip);
    }

    private static int withOpacity(int colour, double opacity) {
        int alpha = (int) Math.round(Color.alpha(colour) * Math.max(0d, Math.min(1d, opacity)));
        return Color.argb(alpha, Color.red(colour), Color.green(colour), Color.blue(colour));
    }

    /** MiniKb 字母键右上角的提示字，与设计的 HINT 表一一对应。 */
    private static final String LETTER_HINTS = "qwertyuiopasdfghjklzxcvbnm";
    private static final String[] HINTS = {"1", "2", "3", "4", "5", "6", "7", "8", "9", "0",
        "@", "#", "¥", "%", "&", "*", "(", ")", "\"", "~", "…", "、", "?", "!", "-", "/"};
    private static final String[][] MINI_ROWS = {
        {"q", "w", "e", "r", "t", "y", "u", "i", "o", "p"},
        {"a", "s", "d", "f", "g", "h", "j", "k", "l"},
        {"⇧", "z", "x", "c", "v", "b", "n", "m", "⌫"},
        {"123", "中", "，", "", "。", "↵"},
    };
    private static final float[][] MINI_WEIGHTS = {
        null, null, {1.4f, 1, 1, 1, 1, 1, 1, 1, 1.4f}, {1.25f, 1.05f, 1, 4, 1, 1.9f},
    };

    /**
     * 在 bounds 上按 MINI_WIDTH 等比缩放画 MiniKb 的工具栏、键区和指示条。尺寸都是设计的虚拟像素：内边距 3、工具栏 50、工具栏与键区间距 8、行高 43、行距 11、键距 6、键角 5、键帽底部 1 px 阴影。
     */
    private static void drawMiniKeyboard(Canvas canvas, RectF bounds, int key, int function,
                                         int returnFill, int returnInk, int ink, int sub, int accent) {
        float sc = bounds.width() / MINI_WIDTH;
        int saved = canvas.save();
        canvas.translate(bounds.left, bounds.top);
        canvas.scale(sc, sc);
        Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
        RectF rect = new RectF();
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
                android.graphics.Path chevron = new android.graphics.Path();
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
        Paint text = new Paint(Paint.ANTI_ALIAS_FLAG);
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
                    text.setTextSize(letter || punctuation ? 22f : 15f);
                    text.setFakeBoldText(special);
                    Paint.FontMetrics metrics = text.getFontMetrics();
                    canvas.drawText(face, rect.centerX(), rect.centerY() - (metrics.ascent + metrics.descent) / 2f, text);
                }
                if (letter) {
                    text.setTextAlign(Paint.Align.RIGHT);
                    text.setColor(sub);
                    text.setTextSize(10f);
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
        paint.setColor(withOpacity(ink, .85));
        rect.set(MINI_WIDTH / 2f - 67f, barTop, MINI_WIDTH / 2f + 67f, barTop + 5f);
        canvas.drawRoundRect(rect, 3f, 3f, paint);
        canvas.restoreToCount(saved);
    }

    /** 瓷砖缩略图每行的键数。 */
    public static int[] tileRowCounts() { return new int[] {5, 5, 3}; }
}
