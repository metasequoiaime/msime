import app.msime.android.SwipeHintPolicy;
import app.msime.android.SymbolSwipe;
import app.msime.android.SymbolSwipe.State;

/** 符号滑动的待命、滑回取消和再次待命（#11822）：阈值严格大于才待命，退回阈值一半以内取消，横向漂移不取消，滑过一次就不再回到点按。 */
public final class SymbolSwipeSmoke {
    private static final String UP = SwipeHintPolicy.UP;
    private static final String DOWN = SwipeHintPolicy.DOWN;
    private static final String EITHER = SymbolSwipe.EITHER;

    public static void main(String[] args) {
        check(SymbolSwipe.travel(UP, -20) == 20 && SymbolSwipe.travel(DOWN, -20) == -20
            && SymbolSwipe.travel(EITHER, -20) == 20 && SymbolSwipe.travel("bogus", 20) == 20, "沿方向的位移");
        check(SymbolSwipe.arms(UP, 14, 0, -14.1f, false) && !SymbolSwipe.arms(UP, 14, 0, -14, false),
            "严格超过阈值才待命");
        check(!SymbolSwipe.arms(UP, 14, 20, -20, true) && SymbolSwipe.arms(UP, 14, 20, -20, false),
            "要求纵向为主时斜滑不待命");
        check(!SymbolSwipe.arms(null, 14, 0, -40, false), "方向为 null 时不响应");
        check(!SymbolSwipe.arms(UP, 14, Float.NaN, -40, false) && !SymbolSwipe.disarms(UP, 14, Float.NaN),
            "无效坐标");
        check(SymbolSwipe.disarms(UP, 14, -7) && !SymbolSwipe.disarms(UP, 14, -7.1f), "退回一半（含）以内才取消");
        check(SymbolSwipe.disarms(UP, 48, -24) && !SymbolSwipe.disarms(UP, 48, -24.5f), "一半随阈值变");
        check(SymbolSwipe.disarms(UP, 14, 30), "滑过起点到反方向也算取消");

        // 上滑：待命，滑回取消，再滑出去又待命。
        SymbolSwipe s = swipe(UP, 14, true);
        check(!s.move(100, 95) && s.state() == State.IDLE && !s.engaged(), "轻微移动不接管");
        check(s.move(100, 85) && s.state() == State.ARMED && s.commits() && s.engaged(), "滑过阈值待命");
        check(!s.move(100, 92) && s.state() == State.ARMED, "防抖区内（阈值和一半之间）保持待命");
        check(!s.move(140, 85) && s.state() == State.ARMED, "待命后横向漂移不取消");
        check(s.move(100, 93) && s.state() == State.DISARMED && !s.commits() && s.engaged(), "退回一半以内取消");
        check(!s.move(100, 87) && s.state() == State.DISARMED, "取消后在防抖区内不再待命");
        check(s.move(100, 85) && s.state() == State.ARMED, "取消后再滑过阈值又待命");
        s.move(100, 140);
        check(s.state() == State.DISARMED && s.engaged(), "滑到反方向也取消，不回到点按");
        check(!s.move(100, 140) && s.state() == State.DISARMED, "反方向再远也只是取消");

        // 再次待命仍要求纵向为主。
        s = swipe(UP, 14, true);
        s.move(100, 80);
        s.move(100, 100);
        check(!s.move(130, 80) && s.state() == State.DISARMED, "再次待命也要纵向为主");

        // 两个方向都算（逗号）。
        s = swipe(EITHER, 14, true);
        check(s.move(100, 116) && s.state() == State.ARMED, "下滑待命");
        check(s.move(100, 105) && s.state() == State.DISARMED, "滑回取消");
        check(s.move(100, 85) && s.state() == State.ARMED, "改成上滑又待命");

        // 不要求纵向为主（叹号出 0）。
        s = swipe(UP, 14, false);
        check(s.move(130, 85) && s.state() == State.ARMED, "不要求纵向为主时斜滑也待命");

        // 复位和新按压。
        s.reset();
        check(s.state() == State.IDLE && !s.engaged(), "复位回到 IDLE");
        s.start(0, 0, DOWN, 30, true);
        check(!s.move(0, 20) && s.move(0, 31) && s.state() == State.ARMED, "新按压按新的方向和阈值");
        check(!s.move(0, 16) && s.move(0, 15) && s.state() == State.DISARMED, "阈值 30 时退到 15 dp 取消");
        System.out.println("Android symbol swipe: arm past the threshold, cancel within half, re-arm passed");
    }

    private static SymbolSwipe swipe(String direction, float threshold, boolean vertical) {
        SymbolSwipe swipe = new SymbolSwipe();
        swipe.start(100, 100, direction, threshold, vertical);
        return swipe;
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
