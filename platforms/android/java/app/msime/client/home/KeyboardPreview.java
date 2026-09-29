package app.msime.client.home;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.RectF;
import android.graphics.drawable.GradientDrawable;
import android.util.AttributeSet;
import android.view.View;
import androidx.annotation.Nullable;
import androidx.core.content.ContextCompat;
import app.msime.client.KeyboardSkin;
import app.msime.client.R;

/**
 * A still picture of the keyboard the user actually has.
 *
 * It is a picture and not the input view: the real one belongs to the InputMethodService, bound to an
 * editor and owning an Engine session, and a second instance of it here would be a second owner. The
 * rows are the faces the host actually ships, so the two do not drift apart silently -- the labels
 * come from arrays that mirror `KeyboardLayout`'s two layers.
 *
 * The skin and the grid come from the shared preferences the card above already reads. Drawing a
 * fixed nine-key grid in fixed colours under a caption that named the user's own 26-key layout and
 * their own skin was a card that contradicted itself.
 */
public final class KeyboardPreview extends View {
    private static final String[][] NINE_KEY_ROWS = {
        {"，", "分词", "ABC", "DEF", "⌫"},
        {"。", "GHI", "JKL", "MNO", "."},
        {"？", "PQRS", "TUV", "WXYZ", "0"},
        {"！", "符", "123", "空格", "中", "换行"},
    };
    private static final String[][] FULL_ROWS = {
        {"Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P"},
        {"A", "S", "D", "F", "G", "H", "J", "K", "L"},
        {"⇧", "Z", "X", "C", "V", "B", "N", "M", "⌫"},
        {"符", "123", "，", "空格", "中", "换行"},
    };

    private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final RectF key = new RectF();
    private String[][] rows = NINE_KEY_ROWS;
    private String caption = "九键";
    @Nullable private KeyboardSkin skin;

    /** Inflated from the keyboard page's layout, so it takes the two-argument constructor. */
    public KeyboardPreview(Context context, AttributeSet attributes) {
        super(context, attributes);
        applyBackground();
    }

    /** Built in code, for the community detail sheet, which has no layout of its own. */
    public KeyboardPreview(Context context) {
        super(context);
        applyBackground();
    }

    /**
     * Draw the user's own keyboard.
     *
     * @param skin the resolved skin, or null to keep the page's own palette
     * @param nineKey whether the grid is three columns rather than 26 keys
     * @param caption the short label in the corner of the candidate strip
     */
    public void setKeyboard(@Nullable KeyboardSkin skin, boolean nineKey, String caption) {
        this.skin = skin;
        this.rows = nineKey ? NINE_KEY_ROWS : FULL_ROWS;
        this.caption = caption == null ? "" : caption;
        applyBackground();
        invalidate();
    }

    private void applyBackground() {
        GradientDrawable background = new GradientDrawable();
        background.setColor(skin == null ? Color.rgb(240, 245, 242) : parse(skin.background()));
        background.setCornerRadius(dp(14));
        setBackground(background);
    }

    private int parse(String colour) {
        try {
            return Color.parseColor(colour);
        } catch (IllegalArgumentException error) {
            return Color.rgb(240, 245, 242);
        }
    }

    private float dp(float value) {
        return value * getResources().getDisplayMetrics().density;
    }

    private int ink() {
        return skin == null ? ContextCompat.getColor(getContext(), R.color.ink)
            : parse(skin.keyForeground());
    }

    private int forest() {
        return skin == null ? ContextCompat.getColor(getContext(), R.color.forest)
            : parse(skin.accent());
    }

    private int cap() {
        return skin == null ? Color.WHITE : parse(skin.keyBackground());
    }

    private int accentLabel() {
        return skin == null ? Color.WHITE : parse(skin.onAccent());
    }

    private int secondary() {
        return skin == null ? ContextCompat.getColor(getContext(), R.color.text_secondary)
            : parse(skin.secondary());
    }

    @Override protected void onDraw(Canvas canvas) {
        float pad = dp(8);
        float gap = dp(5);
        float stripHeight = dp(26);
        float radius = skin == null ? dp(7) : dp((float) skin.cornerRadius());

        paint.setColor(ink());
        paint.setTextSize(dp(12));
        paint.setTextAlign(Paint.Align.LEFT);
        canvas.drawText("ni hao", pad + dp(6), pad + stripHeight * 0.66f, paint);
        paint.setColor(forest());
        canvas.drawText("你好", pad + dp(52), pad + stripHeight * 0.66f, paint);
        paint.setColor(secondary());
        canvas.drawText("你号", pad + dp(84), pad + stripHeight * 0.66f, paint);
        paint.setTextAlign(Paint.Align.RIGHT);
        canvas.drawText(caption, getWidth() - pad - dp(6), pad + stripHeight * 0.66f, paint);

        float top = pad + stripHeight;
        float available = getHeight() - top - pad;
        float rowHeight = (available - gap * (rows.length - 1)) / rows.length;
        paint.setTextAlign(Paint.Align.CENTER);
        for (int r = 0; r < rows.length; r++) {
            String[] row = rows[r];
            float width = (getWidth() - pad * 2 - gap * (row.length - 1)) / (float) row.length;
            float y = top + r * (rowHeight + gap);
            for (int c = 0; c < row.length; c++) {
                float x = pad + c * (width + gap);
                key.set(x, y, x + width, y + rowHeight);
                boolean accent = "换行".equals(row[c]) || "中".equals(row[c]);
                paint.setColor(accent ? forest() : cap());
                canvas.drawRoundRect(key, radius, radius, paint);
                paint.setColor(accent ? accentLabel() : ink());
                paint.setTextSize(dp(row[c].length() > 2 ? 10 : 12));
                canvas.drawText(row[c], key.centerX(),
                    key.centerY() + paint.getTextSize() * 0.36f, paint);
            }
        }
    }
}
