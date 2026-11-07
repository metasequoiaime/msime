package app.msime.android;

import android.content.Context;
import android.view.View;

/**
 * 字母行里不画键的占位格（14/18 键「宽窄」第二排两端的空白，#11051）。指定了所属的键（{@link #setOwner}）时，{@link KeyboardKeyArea} 把落在这一格里的按下交给那个键，和落在键距里的按下一样挪进它的边缘；没有指定时照旧不接收触摸。
 *
 * <p>所属的键必须和占位格在同一排（同一个父视图）里，按下不跨排归属。占位格本身不画、读屏不念：读屏仍按键帽的范围找键。
 */
public final class KeyboardSpacer extends View {
    private View owner;

    public KeyboardSpacer(Context context) {
        super(context);
        ViewPolicy.hideFromAccessibility(this);
    }

    /** 落在这一格里的按下归 `key`；传 null 时这一格不接收触摸。 */
    public void setOwner(View key) {
        owner = key;
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
