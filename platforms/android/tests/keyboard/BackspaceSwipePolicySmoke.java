import app.msime.android.BackspaceSwipePolicy;
import app.msime.android.BackspaceSwipePolicy.Phase;

public final class BackspaceSwipePolicySmoke {
    public static void main(String[] args) {
        float density = 2f;
        // 26 键第三排的删除键：上面还有两排键和工具栏，上沿在 300 px。
        float keyTop = 300f;
        check(BackspaceSwipePolicy.armLine(keyTop, density) == 220f, "arm line sits 40 dp above the key");
        check(BackspaceSwipePolicy.next(Phase.IDLE, keyTop + 10, keyTop, density) == Phase.IDLE,
            "a press inside the key stays a delete");
        check(BackspaceSwipePolicy.next(Phase.IDLE, keyTop - 15, keyTop, density) == Phase.IDLE,
            "a small drift above the key does not reveal the box");
        check(BackspaceSwipePolicy.next(Phase.IDLE, keyTop - 16, keyTop, density) == Phase.REVEALED,
            "8 dp above the key reveals the box");
        check(BackspaceSwipePolicy.next(Phase.REVEALED, 221f, keyTop, density) == Phase.REVEALED,
            "below the arm line the box is not armed");
        check(BackspaceSwipePolicy.next(Phase.REVEALED, 220f, keyTop, density) == Phase.ARMED,
            "reaching the box arms it");
        check(BackspaceSwipePolicy.next(Phase.IDLE, 100f, keyTop, density) == Phase.ARMED,
            "a fast swipe straight into the box arms it");
        check(BackspaceSwipePolicy.next(Phase.ARMED, 250f, keyTop, density) == Phase.REVEALED,
            "sliding back down disarms");
        check(BackspaceSwipePolicy.next(Phase.ARMED, keyTop + 20, keyTop, density) == Phase.REVEALED,
            "sliding back onto the key keeps the box but does not arm");
        check(BackspaceSwipePolicy.clearsOnRelease(Phase.ARMED), "release while armed clears");
        check(!BackspaceSwipePolicy.clearsOnRelease(Phase.REVEALED), "release while not armed does nothing");
        check(!BackspaceSwipePolicy.clearsOnRelease(Phase.IDLE), "an ordinary release clears nothing");

        BackspaceSwipePolicy.Box box = BackspaceSwipePolicy.box(900f, keyTop, 1000f, 1080f, density);
        check(box.bottom() == 220f && box.top() == 140f, "the box ends at the arm line and is 40 dp tall");
        check(box.right() <= 1080f - 8f && box.right() - box.left() == 240f,
            "the box is at least 120 dp wide and stays inside the right edge");
        BackspaceSwipePolicy.Box wide = BackspaceSwipePolicy.box(400f, keyTop, 560f, 1080f, density);
        check(wide.right() - wide.left() == 320f && wide.left() == 320f,
            "the box is two keys wide and centred on the key");

        // 九键的删除键在第一排，上面只有约 50 dp 的工具栏：框缩到至少 28 dp，待命线仍在弹出线之上。
        float topRow = 100f;
        float line = BackspaceSwipePolicy.armLine(topRow, density);
        check(line == 64f, "a top-row key keeps a 28 dp box below the 4 dp margin");
        check(line < topRow - 16f, "arming still needs more travel than revealing");
        BackspaceSwipePolicy.Box tight = BackspaceSwipePolicy.box(980f, topRow, 1060f, 1080f, density);
        check(tight.top() == 8f && tight.bottom() == 64f, "the box is clamped to the surface top");
        // 工具栏隐藏、没在组字时，第一排删除键贴着覆盖层上沿：框画不出来，上滑手势整个不生效，不能不弹框就直接待命。
        for (float cramped : new float[] {0f, 4f, 20f, 60f, 95f}) {
            check(!BackspaceSwipePolicy.available(cramped, density), "no room above the key disables the swipe");
            check(BackspaceSwipePolicy.next(Phase.IDLE, cramped - 16f, cramped, density) == Phase.IDLE,
                "with no room a swipe past the reveal line does not reveal");
            check(BackspaceSwipePolicy.next(Phase.IDLE, -500f, cramped, density) == Phase.IDLE,
                "with no room even a long swipe never arms");
        }
        // 只要能待命，框就至少 28 dp 高，待命线比弹出线至少再高 8 dp。
        check(BackspaceSwipePolicy.available(96f, density), "48 dp above the key is enough room");
        for (float top = 96f; top <= 400f; top += 0.5f) {
            check(BackspaceSwipePolicy.available(top, density), "more room stays available");
            BackspaceSwipePolicy.Box room = BackspaceSwipePolicy.box(980f, top, 1060f, 1080f, density);
            check(room.bottom() - room.top() >= BackspaceSwipePolicy.MIN_BOX_HEIGHT_DP * density,
                "a reachable box is at least 28 dp tall");
            check(room.top() >= BackspaceSwipePolicy.MARGIN_DP * density, "a reachable box stays inside the surface");
            check(BackspaceSwipePolicy.armLine(top, density)
                    <= top - (BackspaceSwipePolicy.REVEAL_DP + BackspaceSwipePolicy.MIN_ARM_TRAVEL_DP) * density,
                "arming needs 8 dp more travel than revealing");
            check(BackspaceSwipePolicy.next(Phase.IDLE, top - BackspaceSwipePolicy.REVEAL_DP * density, top, density)
                    == Phase.REVEALED, "the first step past the reveal line only reveals");
        }

        StringBuilder document = new StringBuilder();
        for (int index = 0; index < 10_000; index++) document.append('哈');
        document.append("😀end");
        int cursor = document.length() - 3;
        StringBuilder after = new StringBuilder(document.substring(cursor));
        StringBuilder before = new StringBuilder(document.substring(0, cursor));
        long deleted = BackspaceSwipePolicy.clearBeforeCursor(editor(before, Integer.MAX_VALUE));
        check(before.length() == 0 && deleted == cursor, "repeated text is cleared in full, chunk by chunk");
        check(after.toString().equals("end"), "text after the cursor is untouched");

        StringBuilder capped = new StringBuilder("abc".repeat(1000));
        BackspaceSwipePolicy.clearBeforeCursor(editor(capped, 100));
        check(capped.length() == 0, "an editor that returns short reads is still cleared");

        StringBuilder stubborn = new StringBuilder("keep");
        int[] reads = {0};
        long none = BackspaceSwipePolicy.clearBeforeCursor(new BackspaceSwipePolicy.Editor() {
            @Override public CharSequence textBeforeCursor(int length) {
                reads[0]++;
                return stubborn;
            }

            @Override public boolean deleteBeforeCursor(int length) { return true; }
        });
        check(reads[0] == BackspaceSwipePolicy.CLEAR_MAX_ROUNDS && none > 0,
            "an editor that claims to delete but does not is bounded by the round limit");
        check(BackspaceSwipePolicy.clearBeforeCursor(new BackspaceSwipePolicy.Editor() {
            @Override public CharSequence textBeforeCursor(int length) { return null; }

            @Override public boolean deleteBeforeCursor(int length) { throw new AssertionError("no read, no delete"); }
        }) == 0, "an editor without text access is left alone");
        System.out.println("Android backspace quick delete swipe passed");
    }

    private static BackspaceSwipePolicy.Editor editor(StringBuilder text, int maximumRead) {
        return new BackspaceSwipePolicy.Editor() {
            @Override public CharSequence textBeforeCursor(int length) {
                int count = Math.min(Math.min(length, maximumRead), text.length());
                return text.substring(text.length() - count);
            }

            @Override public boolean deleteBeforeCursor(int length) {
                text.setLength(Math.max(0, text.length() - length));
                return true;
            }
        };
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
