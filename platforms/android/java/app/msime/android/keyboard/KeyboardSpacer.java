package app.msime.android;

import android.content.Context;
import android.view.View;

/**
 * 字母行里不画键的占位格（14/18 键「宽窄」第二排两端的空白，#11051）。指定了所属的键（{@link #setOwner}）时，{@link KeyboardKeyArea} 把落在这一格或它的外边距里的按下交给那个键，和落在键距里的按下一样挪进它的边缘；没有指定时照旧不接收触摸。
 *
 * <p>所属的键必须和占位格在同一排（同一个父视图）里，按下不跨排归属。占位格本身不画、读屏不念：读屏仍按键帽的范围找键。
 *
 * <p>占位格可以朝外（行首的朝左、行尾的朝右）补几个半键距的外边距（{@link #setOuterHalfGaps}），用来补齐这一排比首排少扣的键距。外边距由套用键距的那一遍（`ImeStyler#applyKeyboardGeometry`）按当前键距换成像素，键距就地变化时跟着变，不需要另外记住占位格。
 */
public final class KeyboardSpacer extends View {
    private View owner;
    private boolean leading;
    private int outerHalfGaps;

    public KeyboardSpacer(Context context) {
        super(context);
        ViewPolicy.hideFromAccessibility(this);
    }

    /** 落在这一格里的按下归 `key`；传 null 时这一格不接收触摸。 */
    public void setOwner(View key) {
        owner = key;
    }

    /** 这一格在行首（`leading`，外边距在左）还是行尾（在右），朝外补 `halfGaps` 个半键距；0 表示不补。 */
    public void setOuterHalfGaps(boolean leading, int halfGaps) {
        this.leading = leading;
        outerHalfGaps = Math.max(0, halfGaps);
    }

    /** 按半个键距 `halfGap` 像素换算出的左、右外边距；上下不留，占位格占满整排高度。 */
    int leftMargin(int halfGap) {
        return leading ? halfGap * outerHalfGaps : 0;
    }

    int rightMargin(int halfGap) {
        return leading ? 0 : halfGap * outerHalfGaps;
    }

    /** 这一格此刻能把按下交给的键：同一排里可见、量过尺寸的那个，否则为 null。 */
    View usableOwner() {
        View key = owner;
        if (key == null || key.getParent() != getParent() || key.getVisibility() != VISIBLE
                || key.getWidth() < 2 || key.getHeight() < 2)
            return null;
        return key;
    }
}
