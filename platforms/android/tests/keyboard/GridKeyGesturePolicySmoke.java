import app.msime.android.GridKeyGesturePolicy;
import app.msime.android.GridKeyGesturePolicy.Mode;
import app.msime.android.SwipeHintPolicy;

/** 14/18 键字母键的手势（#11063）：纵滑出符号、反向或长按开选项、18 键双拼横滑限定字母，组字中纵滑和长按都是点按。 */
public final class GridKeyGesturePolicySmoke {
    private static final String UP = SwipeHintPolicy.UP;
    private static final String DOWN = SwipeHintPolicy.DOWN;

    public static void main(String[] args) {
        check(GridKeyGesturePolicy.vertical(UP, 0, -14.1f), "上滑严格超过 14 dp");
        check(!GridKeyGesturePolicy.vertical(UP, 0, -14), "14 dp 以内不算");
        check(!GridKeyGesturePolicy.vertical(UP, 0, 20), "上滑设置下，下滑不算上滑");
        check(GridKeyGesturePolicy.vertical(DOWN, 0, 15), "下滑设置");
        check(!GridKeyGesturePolicy.vertical(UP, 20, -20) && GridKeyGesturePolicy.vertical(UP, 19, -20),
            "纵向位移必须大于横向");
        check(!GridKeyGesturePolicy.vertical(null, 0, -40), "滑动输入符号关闭");
        check(!GridKeyGesturePolicy.vertical(UP, Float.NaN, -40), "无效坐标");
        check(DOWN.equals(GridKeyGesturePolicy.opposite(UP)) && UP.equals(GridKeyGesturePolicy.opposite(DOWN))
            && GridKeyGesturePolicy.opposite(null) == null, "反方向");

        // 空闲时上滑：锁定为符号，回到起点仍输入符号。
        GridKeyGesturePolicy.Gesture g = gesture(null, UP, false);
        check(!g.move(1, 1, 100, 95) && g.mode() == Mode.PENDING && !g.consumed(), "轻微移动仍是点按");
        check(g.move(1, 1, 100, 80) && g.mode() == Mode.SYMBOL && g.consumed(), "上滑接管为符号");
        g.move(1, 1, 100, 100);
        g.move(1, 1, 140, 100);
        check(g.mode() == Mode.SYMBOL && !g.hold(), "符号锁定：回中心、横移和长按都不改");
        check(!g.forcesTap(), "符号不补点按");

        // 反方向打开选项；下滑设置时方向对调。
        g = gesture(null, UP, false);
        check(g.move(1, 1, 100, 120) && g.mode() == Mode.OPTIONS, "反向下滑打开选项");
        g.move(1, 1, 100, 70);
        check(g.mode() == Mode.OPTIONS, "选项打开后移动交给拖选");
        g = gesture(null, DOWN, false);
        g.move(1, 1, 100, 120);
        check(g.mode() == Mode.SYMBOL, "下滑设置：下滑出符号");
        g = gesture(null, DOWN, false);
        g.move(1, 1, 100, 80);
        check(g.mode() == Mode.OPTIONS, "下滑设置：上滑打开选项");

        // 开关关闭：两种纵滑都不响应，长按照旧。
        g = gesture(null, null, false);
        g.move(1, 1, 100, 60);
        g.move(1, 1, 100, 140);
        check(g.mode() == Mode.PENDING, "关闭时纵滑不触发");
        check(g.hold() && g.mode() == Mode.OPTIONS, "关闭时长按照旧打开选项");

        // 空闲长按。
        g = gesture(null, UP, false);
        check(g.hold() && g.mode() == Mode.OPTIONS && g.consumed(), "空闲长按打开选项");
        check(!g.hold(), "长按只触发一次");
        g = gesture("we", UP, false);
        g.hold();
        g.move(1, 1, 135, 100);
        check(g.mode() == Mode.OPTIONS && g.selected() == 0, "长按已弹选项后不抢成横滑");

        // 组字中：纵滑和长按都是点按，横滑照常。
        g = gesture(null, UP, true);
        g.move(1, 1, 100, 60);
        g.move(1, 1, 100, 140);
        check(g.mode() == Mode.PENDING && !g.hold(), "组字中纵滑、长按都不触发");
        check(g.forcesTap(), "组字中松手补一次点按");
        g = gesture("we", UP, true);
        g.move(1, 1, 70, 100);
        check(g.mode() == Mode.LETTER && g.selected() == 'W' && !g.forcesTap(), "组字中横滑限定字母");
        g = gesture(null, UP, false);
        g.composing();
        g.move(1, 1, 100, 60);
        check(g.mode() == Mode.PENDING && g.forcesTap(), "按压中途开始组字后按点按处理");
        g = gesture(null, UP, false);
        g.move(1, 1, 100, 60);
        g.composing();
        check(g.mode() == Mode.SYMBOL, "已触发的符号不因之后开始组字而取消");

        // 18 键双拼横滑：反向重选，回中心不输入；横滑锁定后纵滑不改。
        g = gesture("we", UP, false);
        check(g.move(1, 1, 125.5f, 100) && g.mode() == Mode.LETTER && g.selected() == 'E', "右滑取右字母");
        check(g.move(1, 1, 74, 100) && g.selected() == 'W', "反向重选");
        g.move(1, 1, 100, 100);
        check(g.mode() == Mode.LETTER && g.selected() == 0 && g.consumed(), "回中心不输入，仍接管");
        g.move(1, 1, 100, 60);
        check(g.mode() == Mode.LETTER && !g.hold(), "横滑锁定后纵滑、长按不改");
        g = gesture("we", UP, false);
        g.move(1, 1, 130, 85);
        check(g.mode() == Mode.LETTER && g.selected() == 'E', "明显横滑允许少量纵移");
        g = gesture("we", UP, false);
        g.move(1, 1, 110, 80);
        check(g.mode() == Mode.SYMBOL, "纵向为主的斜滑出符号");
        g = gesture("we", UP, false);
        g.move(1, 1, 100, 85);
        g.move(1, 1, 130, 85);
        check(g.mode() == Mode.SYMBOL, "先触发的纵滑锁定，之后的横移不改");
        g = gesture(null, UP, false);
        g.move(1, 1, 140, 100);
        check(g.mode() == Mode.PENDING, "没有横滑的键横移仍是点按");

        // 没有符号的键：只有选项。
        g = new GridKeyGesturePolicy.Gesture(null, false);
        g.start(1, 100, 100, UP, false);
        g.move(1, 1, 100, 60);
        check(g.mode() == Mode.PENDING, "没有符号时上滑不触发");

        // 多指、换指、取消。
        g = gesture("we", UP, false);
        g.move(1, 2, 100, 100);
        g.move(1, 1, 130, 100);
        check(g.mode() == Mode.CANCELLED && g.selected() == 0 && !g.forcesTap(), "多指后不输入");
        g = gesture(null, UP, false);
        g.move(1, 1, 100, 60);
        g.move(2, 1, 100, 60);
        check(g.mode() == Mode.CANCELLED, "符号触发后换指也取消");
        g = gesture(null, UP, true);
        g.cancel();
        check(g.mode() == Mode.CANCELLED && !g.forcesTap() && !g.hold(), "系统取消不补点按");
        g.start(1, 100, 100, UP, false);
        check(g.mode() == Mode.PENDING && g.selected() == 0 && g.hold(), "新按压不继承取消和组字状态");
        System.out.println("Android 14/18-key letter gestures: passed");
    }

    private static GridKeyGesturePolicy.Gesture gesture(String exact, String direction, boolean composing) {
        GridKeyGesturePolicy.Gesture gesture = new GridKeyGesturePolicy.Gesture(exact, true);
        gesture.start(1, 100, 100, direction, composing);
        return gesture;
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
