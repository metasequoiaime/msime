package app.msime.android;

import app.msime.android.KeyboardAdjustPolicy.Availability;
import app.msime.android.KeyboardAdjustPolicy.Horizontal;
import app.msime.android.KeyboardAdjustPolicy.Part;
import app.msime.android.KeyboardAdjustPolicy.Range;
import app.msime.android.KeyboardAdjustPolicy.Vertical;

/**
 * 「键盘调整」遮罩的纯逻辑：哪些柄显示、每条边拖动只动自己那条边（对边在屏幕上不动）、抬高与宽度位置的换算、读屏范围和步长。
 */
public final class KeyboardAdjustPolicySmoke {
    /** 一台约 800 dp 高的竖屏手机：窗口封顶不低于设计上限 129，整段 75–170% 都能拖。 */
    private static final int TALL_WINDOW_DP = 800;
    /** 横屏或分屏的矮窗口：17% 是 61 dp，130% 以上被封顶。 */
    private static final int SHORT_WINDOW_DP = 360;
    /** 1080 px 宽、2.75 倍密度的手机：240 dp 是 660 px，比 60% 的 648 px 宽。 */
    private static final int PHONE_PX = 1080;
    private static final int PHONE_MIN_PX = 660;

    public static void main(String[] args) {
        availability();
        topHandle();
        bottomHandle();
        move();
        widthGeometry();
        sideHandles();
        accessibility();
        System.out.println("KeyboardAdjustPolicySmoke ok");
    }

    private static void availability() {
        Availability docked = Availability.of(false, false, false, false, false);
        for (Part part : Part.values()) check(docked.shows(part), "a docked keyboard shows " + part);
        check(docked.movesHorizontally(), "a docked keyboard moves both ways");
        for (Availability onlyTop : new Availability[] {
                Availability.of(true, false, false, false, false),
                Availability.of(false, true, false, false, false),
                Availability.of(false, false, true, false, false)}) {
            check(onlyTop.shows(Part.TOP), "the top handle is always there");
            for (Part part : new Part[] {Part.BOTTOM, Part.LEFT, Part.RIGHT, Part.MOVE})
                check(!onlyTop.shows(part), "floating, collapsed keys and phone landscape hide " + part);
            check(!onlyTop.lift() && !onlyTop.width(), "nothing but the height applies there");
        }
        for (Availability narrowOff : new Availability[] {
                Availability.of(false, false, false, true, false),
                Availability.of(false, false, false, false, true)}) {
            check(narrowOff.shows(Part.TOP) && narrowOff.shows(Part.BOTTOM) && narrowOff.shows(Part.MOVE),
                "one-handed and split keep the height, lift and move");
            check(!narrowOff.shows(Part.LEFT) && !narrowOff.shows(Part.RIGHT), "one-handed and split hide the side handles");
            check(!narrowOff.movesHorizontally(), "move only lifts there");
        }
    }

    private static void topHandle() {
        check(KeyboardAdjustPolicy.dragTop(0, -20, TALL_WINDOW_DP) == 20, "dragging up 20 dp grows 20 dp");
        check(KeyboardAdjustPolicy.dragTop(0, 30, TALL_WINDOW_DP) == -30, "dragging down shrinks");
        check(KeyboardAdjustPolicy.dragTop(0, -1000, TALL_WINDOW_DP) == 129, "the top stops at 170%");
        check(KeyboardAdjustPolicy.dragTop(0, 1000, TALL_WINDOW_DP) == -46, "the top stops at 75%");
        check(KeyboardAdjustPolicy.maxHeightAdjustment(SHORT_WINDOW_DP) == 61, "a short window caps at 17%");
        check(KeyboardAdjustPolicy.dragTop(0, -1000, SHORT_WINDOW_DP) == 61, "a short window stops at its cap");
        // 存着 129 而窗口只画得出 61：往下拖 10 dp 立刻变矮，不先走完画不出来的那一段。
        check(KeyboardAdjustPolicy.dragTop(129, 10, SHORT_WINDOW_DP) == 51, "dragging starts from the drawn height");
    }

    private static void bottomHandle() {
        // 往上拖 d：抬高加 d、高度减 d，上沿不动（两者之和不变）。
        Vertical up = KeyboardAdjustPolicy.dragBottom(20, 0, -16, TALL_WINDOW_DP, false);
        check(up.liftDp() == 16 && up.heightAdjustment() == 4, "lifting 16 dp takes 16 dp of height: " + up);
        for (int dy = -200; dy <= 200; dy += 7) {
            Vertical next = KeyboardAdjustPolicy.dragBottom(20, 40, dy, TALL_WINDOW_DP, false);
            int u = -dy;
            if (u <= 66) {
                // 高度还没让到下限（20 − 66 = −46）：上沿不动。往下拖时抬高到 0 就停，两者之和同样不变。
                check(next.liftDp() + next.heightAdjustment() == 60, "the top edge stays put at dy " + dy + ": " + next);
            } else {
                // 高度到了下限：高度停在 75%，抬高继续跟着手指走，整块连同上沿一起上移，到 160 dp 停。
                check(next.heightAdjustment() == -46 && next.liftDp() == Math.min(40 + u, 160),
                    "past the height floor the lift keeps following the finger at dy " + dy + ": " + next);
            }
            check(next.liftDp() >= 0 && next.liftDp() <= 160, "lift stays in range at dy " + dy);
            check(next.heightAdjustment() >= -46 && next.heightAdjustment() <= 129, "height stays in range at dy " + dy);
        }
        Vertical atFloor = KeyboardAdjustPolicy.dragBottom(-46, 20, -30, TALL_WINDOW_DP, false);
        check(atFloor.heightAdjustment() == -46 && atFloor.liftDp() == 50,
            "with the height at 75% dragging up keeps lifting and the height stays at the floor: " + atFloor);
        Vertical crossing = KeyboardAdjustPolicy.dragBottom(-30, 0, -40, TALL_WINDOW_DP, false);
        check(crossing.heightAdjustment() == -46 && crossing.liftDp() == 40,
            "one drag across the floor gives up 16 dp of height, then moves the whole keyboard up: " + crossing);
        Vertical floor = KeyboardAdjustPolicy.dragBottom(-40, 0, -100, TALL_WINDOW_DP, false);
        check(floor.heightAdjustment() == -46 && floor.liftDp() == 100, "lifting goes on past the height floor: " + floor);
        Vertical back = KeyboardAdjustPolicy.dragBottom(-46, 50, 30, TALL_WINDOW_DP, false);
        check(back.liftDp() == 20 && back.heightAdjustment() == -16,
            "dragging back down still trades lift for height with the top fixed: " + back);
        Vertical ground = KeyboardAdjustPolicy.dragBottom(0, 10, 100, TALL_WINDOW_DP, false);
        check(ground.liftDp() == 0 && ground.heightAdjustment() == 10, "lowering stops at no lift: " + ground);
        Vertical ceiling = KeyboardAdjustPolicy.dragBottom(100, 150, -100, TALL_WINDOW_DP, false);
        check(ceiling.liftDp() == 160 && ceiling.heightAdjustment() == 90, "lifting stops at 160 dp: " + ceiling);
        check(KeyboardAdjustPolicy.maxLiftDp(true) == 120, "with the bottom bar shown 40 of the 160 dp are the bar's");
        Vertical barred = KeyboardAdjustPolicy.dragBottom(100, 110, -100, TALL_WINDOW_DP, true);
        check(barred.liftDp() == 120 && barred.heightAdjustment() == 90, "the bar shortens the lift range: " + barred);
        Vertical capped = KeyboardAdjustPolicy.dragBottom(50, 20, 100, SHORT_WINDOW_DP, false);
        check(capped.heightAdjustment() == 61 && capped.liftDp() == 9, "lowering stops at the window's height cap: " + capped);

        check(KeyboardAdjustPolicy.liftDp(true, 96, false) == 96 && KeyboardAdjustPolicy.liftDp(true, 96, true) == 56,
            "the stored target counts the bar's 40 dp");
        check(KeyboardAdjustPolicy.liftDp(true, 30, true) == 0 && KeyboardAdjustPolicy.liftDp(false, 96, false) == 0,
            "a target under the bar or a switch that is off lifts nothing");
        check(KeyboardAdjustPolicy.storedBottomHeightDp(56, true) == 96 && KeyboardAdjustPolicy.storedBottomHeightDp(56, false) == 56
            && KeyboardAdjustPolicy.storedBottomHeightDp(0, true) == 0, "the lift is stored as the target total");
        for (boolean bar : new boolean[] {false, true}) {
            for (int lift = 0; lift <= KeyboardAdjustPolicy.maxLiftDp(bar); lift++) {
                int stored = KeyboardAdjustPolicy.storedBottomHeightDp(lift, bar);
                check(stored <= KeyboardBottomBarPolicy.MAX_PADDING_DP, "stored target fits the setting at " + lift);
                check(KeyboardAdjustPolicy.liftDp(stored > 0, stored, bar) == lift, "lift round trip at " + lift + " bar " + bar);
            }
        }
    }

    private static void move() {
        check(KeyboardAdjustPolicy.dragLift(20, -30, false) == 50, "move up lifts");
        check(KeyboardAdjustPolicy.dragLift(20, 30, false) == 0 && KeyboardAdjustPolicy.dragLift(100, -100, true) == 120,
            "move clamps the lift");
        Horizontal narrow = new Horizontal(80, 0);
        int width = KeyboardAdjustPolicy.widthPx(80, PHONE_PX, PHONE_MIN_PX);
        Horizontal moved = KeyboardAdjustPolicy.dragPosition(narrow, 108, PHONE_PX, PHONE_MIN_PX);
        check(moved.widthPercent() == 80 && KeyboardAdjustPolicy.leftPx(moved.position(), PHONE_PX, width) == 108,
            "move keeps the width and follows the finger: " + moved);
        check(KeyboardAdjustPolicy.dragPosition(narrow, 5000, PHONE_PX, PHONE_MIN_PX).position() == 1000
            && KeyboardAdjustPolicy.dragPosition(narrow, -5000, PHONE_PX, PHONE_MIN_PX).position() == 0, "move stops at the sides");
        check(KeyboardAdjustPolicy.dragPosition(Horizontal.DEFAULT, 300, PHONE_PX, PHONE_MIN_PX).equals(Horizontal.DEFAULT),
            "a full-width keyboard has nowhere to move");
    }

    private static void widthGeometry() {
        // 最窄的键盘里，中间三个按钮加左右两个竖向把手必须并排放得下，按钮不压到柄上（三个按钮之间没有间距）。60% 只会比 240 dp 更宽，浮动键盘最窄也是 240 dp 且不显示左右柄。
        check(3 * KeyboardAdjustOverlay.ACTION_WIDTH_DP + 2 * KeyboardAdjustOverlay.GRIP_SHORT_DP
            <= KeyboardAdjustPolicy.MIN_WIDTH_DP, "the three buttons and both side grips fit the narrowest keyboard");
        check(KeyboardAdjustPolicy.minWidthPx(PHONE_PX, PHONE_MIN_PX) == 660, "240 dp beats 60% on a phone");
        check(KeyboardAdjustPolicy.minWidthPx(2000, 660) == 1200, "60% beats 240 dp on a wide surface");
        check(KeyboardAdjustPolicy.minWidthPx(500, 660) == 500, "never wider than the space");
        check(KeyboardAdjustPolicy.widthPx(60, PHONE_PX, PHONE_MIN_PX) == 660, "a stored 60% still draws 240 dp");
        check(KeyboardAdjustPolicy.widthPx(100, PHONE_PX, PHONE_MIN_PX) == PHONE_PX, "100% fills the space");
        int[] centred = KeyboardAdjustPolicy.insetsPx(new Horizontal(80, 500), 1000, 0);
        check(centred[0] == 100 && centred[1] == 100, "80% centred leaves 100 px each side");
        int[] left = KeyboardAdjustPolicy.insetsPx(new Horizontal(80, 0), 1000, 0);
        check(left[0] == 0 && left[1] == 200, "position 0 hugs the left");
        int[] right = KeyboardAdjustPolicy.insetsPx(new Horizontal(80, 1000), 1000, 0);
        check(right[0] == 200 && right[1] == 0, "position 1000 hugs the right");
        int[] full = KeyboardAdjustPolicy.insetsPx(Horizontal.DEFAULT, 1000, 0);
        check(full[0] == 0 && full[1] == 0, "the default leaves no gutters");
        check(KeyboardAdjustPolicy.insetsPx(new Horizontal(80, 500), 0, 0)[0] == 0, "an unmeasured surface has no gutters");
    }

    private static void sideHandles() {
        Horizontal start = new Horizontal(80, 500);
        int startWidth = KeyboardAdjustPolicy.widthPx(80, 1000, 0);
        int startLeft = KeyboardAdjustPolicy.leftPx(500, 1000, startWidth);
        int startRight = startLeft + startWidth;
        for (int dx = -400; dx <= 400; dx += 10) {
            Horizontal right = KeyboardAdjustPolicy.dragRight(start, dx, 1000, 0);
            int width = KeyboardAdjustPolicy.widthPx(right.widthPercent(), 1000, 0);
            int left = KeyboardAdjustPolicy.leftPx(right.position(), 1000, width);
            check(Math.abs(left - startLeft) <= 1, "the right handle keeps the left edge at dx " + dx + ": " + right);
            check(left + width <= 1000 && width >= 600, "the right handle stays inside and above 60% at dx " + dx);
            Horizontal leftDrag = KeyboardAdjustPolicy.dragLeft(start, dx, 1000, 0);
            int width2 = KeyboardAdjustPolicy.widthPx(leftDrag.widthPercent(), 1000, 0);
            int left2 = KeyboardAdjustPolicy.leftPx(leftDrag.position(), 1000, width2);
            check(Math.abs(left2 + width2 - startRight) <= 1, "the left handle keeps the right edge at dx " + dx + ": " + leftDrag);
            check(left2 >= 0 && width2 >= 600, "the left handle stays inside and above 60% at dx " + dx);
        }
        Horizontal shrunk = KeyboardAdjustPolicy.dragRight(start, -50, 1000, 0);
        check(shrunk.widthPercent() == 75, "dragging the right edge in narrows: " + shrunk);
        Horizontal fromFull = KeyboardAdjustPolicy.dragLeft(Horizontal.DEFAULT, 200, 1000, 0);
        check(fromFull.widthPercent() == 80 && fromFull.position() == 1000,
            "narrowing a full keyboard from the left leaves it against the right: " + fromFull);
        Horizontal fromFullRight = KeyboardAdjustPolicy.dragRight(Horizontal.DEFAULT, -200, 1000, 0);
        check(fromFullRight.widthPercent() == 80 && fromFullRight.position() == 0,
            "narrowing a full keyboard from the right leaves it against the left: " + fromFullRight);
        Horizontal floor = KeyboardAdjustPolicy.dragRight(Horizontal.DEFAULT, -900, PHONE_PX, PHONE_MIN_PX);
        check(KeyboardAdjustPolicy.widthPx(floor.widthPercent(), PHONE_PX, PHONE_MIN_PX) >= PHONE_MIN_PX,
            "a phone keyboard never gets narrower than 240 dp: " + floor);
        check(KeyboardAdjustPolicy.dragRight(start, 10, 0, 0).equals(start), "an unmeasured surface keeps the values");
    }

    private static void accessibility() {
        Range height = KeyboardAdjustPolicy.heightRange(0, TALL_WINDOW_DP);
        check(height.min() == 75 && height.max() == 170 && height.current() == 100, "height range in percent: " + height);
        check(KeyboardAdjustPolicy.heightRange(0, SHORT_WINDOW_DP).max() < 170, "a short window caps the reachable percent");
        int step = KeyboardAdjustPolicy.stepHeight(0, 1, TALL_WINDOW_DP);
        check(KeyboardHeightPolicy.heightAdjustmentToPercent(step) == 105, "one step up is 5%");
        int walked = 0;
        for (int i = 0; i < 30; i++) walked = KeyboardAdjustPolicy.stepHeight(walked, 1, TALL_WINDOW_DP);
        check(walked == 129, "stepping up ends at 170%");
        for (int i = 0; i < 30; i++) walked = KeyboardAdjustPolicy.stepHeight(walked, -1, TALL_WINDOW_DP);
        check(walked == -46, "stepping down ends at 75%");
        for (int percent = 75; percent <= 170; percent += 5) {
            int adjustment = KeyboardHeightPolicy.heightPercentToAdjustment(percent);
            check(KeyboardAdjustPolicy.heightRange(adjustment, TALL_WINDOW_DP).current() == percent, "percent round trip " + percent);
        }

        Vertical lifted = KeyboardAdjustPolicy.stepBottom(20, 0, 1, TALL_WINDOW_DP, false);
        check(lifted.liftDp() == 4 && lifted.heightAdjustment() == 16, "one lift step is 4 dp, taken from the height: " + lifted);
        Range bottom = KeyboardAdjustPolicy.bottomRange(20, 0, TALL_WINDOW_DP, false);
        check(bottom.min() == 0 && bottom.max() == 160 && bottom.current() == 0 && bottom.canIncrease() && !bottom.canDecrease(),
            "bottom range: " + bottom);
        Range floorRange = KeyboardAdjustPolicy.bottomRange(-46, 20, TALL_WINDOW_DP, false);
        check(floorRange.canIncrease() && floorRange.canDecrease(),
            "lifting stays available while the height is at 75%: " + floorRange);
        Vertical floorStep = KeyboardAdjustPolicy.stepBottom(-46, 20, 1, TALL_WINDOW_DP, false);
        check(floorStep.liftDp() == 24 && floorStep.heightAdjustment() == -46,
            "a lift step at 75% raises the whole keyboard: " + floorStep);
        check(!KeyboardAdjustPolicy.bottomRange(-46, 160, TALL_WINDOW_DP, false).canIncrease(),
            "no lifting past 160 dp");
        check(KeyboardAdjustPolicy.liftRange(120, true).equals(new Range(0, 120, 120, false, true)), "lift range with the bar");

        Range width = KeyboardAdjustPolicy.widthRange(Horizontal.DEFAULT, PHONE_PX, PHONE_MIN_PX);
        check(width.min() == 62 && width.max() == 100 && width.current() == 100 && !width.canIncrease(),
            "the width range starts at what 240 dp is in percent: " + width);
        Horizontal narrower = KeyboardAdjustPolicy.stepWidth(Horizontal.DEFAULT, Part.RIGHT, -1, 1000, 0);
        check(narrower.widthPercent() == 95 && narrower.position() == 0, "one width step from the right is 5%: " + narrower);
        Horizontal narrowerLeft = KeyboardAdjustPolicy.stepWidth(Horizontal.DEFAULT, Part.LEFT, -1, 1000, 0);
        check(narrowerLeft.widthPercent() == 95 && narrowerLeft.position() == 1000,
            "one width step from the left keeps the right edge: " + narrowerLeft);
        // 柄只动自己那条边：贴着哪一侧，那一侧的柄就不能再加宽，读屏不该提供一个做了没反应的「增」。
        Horizontal hugLeft = new Horizontal(80, 0);
        Horizontal hugRight = new Horizontal(80, 1000);
        Range leftAtLeft = KeyboardAdjustPolicy.widthRange(hugLeft, Part.LEFT, 1000, 0);
        check(!leftAtLeft.canIncrease() && leftAtLeft.canDecrease() && leftAtLeft.current() == 80,
            "against the left the left handle can only narrow: " + leftAtLeft);
        check(KeyboardAdjustPolicy.stepWidth(hugLeft, Part.LEFT, 1, 1000, 0).equals(hugLeft),
            "a widening step from the left does nothing against the left");
        check(KeyboardAdjustPolicy.widthRange(hugLeft, Part.RIGHT, 1000, 0).canIncrease(),
            "against the left the right handle still widens");
        check(!KeyboardAdjustPolicy.widthRange(hugRight, Part.RIGHT, 1000, 0).canIncrease()
            && KeyboardAdjustPolicy.widthRange(hugRight, Part.LEFT, 1000, 0).canIncrease(),
            "against the right only the left handle widens");
        Range centredLeft = KeyboardAdjustPolicy.widthRange(new Horizontal(80, 500), Part.LEFT, 1000, 0);
        check(centredLeft.canIncrease() && centredLeft.canDecrease(), "a centred keyboard widens from either side");
        for (Part side : new Part[] {Part.LEFT, Part.RIGHT}) {
            Range full = KeyboardAdjustPolicy.widthRange(Horizontal.DEFAULT, side, PHONE_PX, PHONE_MIN_PX);
            check(!full.canIncrease() && full.canDecrease() && full.min() == 62, "a full keyboard only narrows: " + full);
            Range floor = KeyboardAdjustPolicy.widthRange(new Horizontal(62, 0), side, PHONE_PX, PHONE_MIN_PX);
            check(!floor.canDecrease(), "the narrowest keyboard offers no narrowing step from the " + side);
        }
        check(KeyboardAdjustPolicy.stepPosition(new Horizontal(80, 500), 1).position() == 550
            && KeyboardAdjustPolicy.stepPosition(new Horizontal(80, 980), 1).position() == 1000, "position steps are 50 per mille");
        Range position = KeyboardAdjustPolicy.positionRange(Horizontal.DEFAULT, 1000, 0);
        check(!position.canIncrease() && !position.canDecrease(), "a full-width keyboard offers no position steps");
        check(KeyboardAdjustPolicy.positionRange(new Horizontal(80, 0), 1000, 0).canIncrease(), "a narrow keyboard can move right");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
