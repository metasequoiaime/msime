import app.msime.android.ClipboardSwipePolicy;
import app.msime.android.KeyboardIconKey;
import app.msime.android.KeyboardIconPaths;

/** #5962：剪贴板本机历史的左滑删除，手势判定不能和面板的纵向滚动、点按插入、长按管理抢。 */
public final class ClipboardSwipePolicySmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        float density = 2.75f;
        float slop = 8 * density;
        float reveal = ClipboardSwipePolicy.revealWidth(density, 0);

        // 删除区宽度：默认 72 dp；双列等窄格里不超过格宽一半。
        check(reveal == ClipboardSwipePolicy.REVEAL_DP * density, "an unmeasured cell reveals the default width");
        check(ClipboardSwipePolicy.revealWidth(density, 1000) == reveal, "a wide cell reveals the default width");
        check(ClipboardSwipePolicy.revealWidth(density, 100) == 50f, "a narrow cell reveals at most half its width");
        try {
            ClipboardSwipePolicy.revealWidth(0, 100);
            throw new AssertionError("a zero density is refused");
        } catch (IllegalArgumentException expected) {
            // 密度来自系统，非正数说明调用方传错了。
        }

        // 没越过 touch slop：仍是点按或长按，不接手。
        check(!ClipboardSwipePolicy.claims(-slop, 0, slop, false), "a move within the slop stays a tap");
        check(!ClipboardSwipePolicy.claims(-slop / 2, 1, slop, false), "a small wobble stays a tap");
        // 明显向左：接手。
        check(ClipboardSwipePolicy.claims(-slop - 1, 0, slop, false), "a leftward drag past the slop swipes");
        check(ClipboardSwipePolicy.claims(-60, 20, slop, false), "a mostly horizontal drag swipes");
        // 纵向和斜向留给面板滚动。
        check(!ClipboardSwipePolicy.claims(0, -80, slop, false), "a vertical drag scrolls the panel");
        check(!ClipboardSwipePolicy.claims(-40, 40, slop, false), "a diagonal drag scrolls the panel");
        check(!ClipboardSwipePolicy.claims(-40, -30, slop, false), "a steep drag scrolls the panel");
        // 收着的卡片向右拖不触发；打开着的向右拖是收回，向左也照样跟手。
        check(!ClipboardSwipePolicy.claims(60, 0, slop, false), "a rightward drag on a closed card does nothing");
        check(ClipboardSwipePolicy.claims(60, 0, slop, true), "a rightward drag on an open card closes it");
        check(ClipboardSwipePolicy.claims(-60, 0, slop, true), "a leftward drag on an open card is still tracked");
        check(!ClipboardSwipePolicy.claims(5, 0, slop, true), "an open card still treats a small move as a tap");

        // 跟手的位移夹在 [-reveal, 0]。
        check(ClipboardSwipePolicy.offset(0, -30, reveal) == -30, "the card follows the finger");
        check(ClipboardSwipePolicy.offset(0, -10_000, reveal) == -reveal, "the card never slides past the delete area");
        check(ClipboardSwipePolicy.offset(0, 50, reveal) == 0, "a closed card never slides right");
        check(ClipboardSwipePolicy.offset(-reveal, 40, reveal) == -reveal + 40, "an open card follows a drag back");
        check(ClipboardSwipePolicy.offset(-reveal, 10_000, reveal) == 0, "an open card dragged far right closes fully");

        // 松手：慢拖按位置（过半打开），快甩按方向。
        float fling = ClipboardSwipePolicy.FLING_DP_PER_SECOND * density;
        check(ClipboardSwipePolicy.settlesOpen(-reveal * .6f, 0, reveal, density), "past halfway opens");
        check(!ClipboardSwipePolicy.settlesOpen(-reveal * .4f, 0, reveal, density), "short of halfway springs back");
        check(ClipboardSwipePolicy.settlesOpen(-reveal * .2f, -fling, reveal, density), "a quick flick left opens");
        check(!ClipboardSwipePolicy.settlesOpen(-reveal * .8f, fling, reveal, density), "a quick flick right closes");
        check(ClipboardSwipePolicy.settlesOpen(-reveal * .8f, fling / 2, reveal, density),
            "a slow drift right keeps a mostly open card open");
        check(!ClipboardSwipePolicy.settlesOpen(0, 0, reveal, density), "an untouched card stays closed");

        // 删除区画垃圾桶。
        check(KeyboardIconKey.iconFor(KeyboardIconKey.Kind.TRASH) == KeyboardIconPaths.Icon.TRASH, "the delete area draws a trash can");
        check(KeyboardIconPaths.Icon.TRASH.stroked(), "the trash can is an outline like the other panel icons");
        System.out.println("ClipboardSwipePolicySmoke ok");
    }
}
