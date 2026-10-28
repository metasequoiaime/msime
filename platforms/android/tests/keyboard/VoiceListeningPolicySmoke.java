import app.msime.android.VoiceListeningPolicy;
import app.msime.android.VoiceListeningPolicy.Plan;

/** 聆听面板的排布、音量平滑和中间文字裁剪；数值与 iOS、HarmonyOS 的同名策略一致。 */
public final class VoiceListeningPolicySmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    private static boolean near(float a, float b) {
        return Math.abs(a - b) < 1e-3f;
    }

    /** 光圈满音量时整块内容（含上下边距）不超出面板。 */
    private static void fitsInside(Plan plan, float height, float titleLine, float hintLine, String message) {
        float bottom = plan.titleTop(1f) + plan.titleLines() * titleLine
            + (plan.showsHint() ? VoiceListeningPolicy.GAP_TITLE_HINT_DP + hintLine : 0f)
            + VoiceListeningPolicy.PAD_BOTTOM_DP;
        float haloTop = plan.indicatorCenterY(1f) - plan.orb() / 2f - plan.halo();
        check(haloTop >= -1e-3f, message + ": halo top " + haloTop);
        check(bottom <= height + 1e-3f, message + ": bottom " + bottom + " > " + height);
    }

    public static void main(String[] args) {
        float title = 21f;
        float hint = 17f;

        // 高的面板：72 dp 圆盘、两行标题、带提示，多出来的高度上下平分。
        Plan tall = VoiceListeningPolicy.plan(260f, title, hint, 1f);
        check(near(tall.orb(), 72f) && near(tall.halo(), 18f), "a tall panel gets the full orb: " + tall);
        check(tall.titleLines() == 2 && tall.showsHint(), "a tall panel reserves two title lines: " + tall);
        fitsInside(tall, 260f, title, hint, "tall");
        check(tall.top() > 0f, "spare height is split around the content");

        // Pixel 3 XL 竖屏的聆听区约 142 dp：两行时圆盘小于 52 dp，退到一行。
        Plan phone = VoiceListeningPolicy.plan(142f, title, hint, 1f);
        check(phone.titleLines() == 1 && phone.showsHint(), "a phone panel drops to one title line: " + phone);
        check(phone.orb() >= VoiceListeningPolicy.ORB_MIN_DP && phone.orb() <= 72f, "phone orb in range: " + phone);
        check(near(phone.halo(), phone.orb() * VoiceListeningPolicy.HALO_RATIO), "phone halo keeps its ratio");
        fitsInside(phone, 142f, title, hint, "phone");

        // 更矮时先藏提示行，再把圆盘停在最小值、收窄光圈，最后才缩小圆盘。
        Plan short_ = VoiceListeningPolicy.plan(100f, title, hint, 1f);
        check(!short_.showsHint() && short_.titleLines() == 1, "a short panel hides the hint: " + short_);
        fitsInside(short_, 100f, title, hint, "short");
        Plan squeezed = VoiceListeningPolicy.plan(85f, title, hint, 1f);
        check(near(squeezed.orb(), VoiceListeningPolicy.ORB_MIN_DP) && squeezed.halo() < 10f,
            "the orb stops at its minimum and the halo narrows: " + squeezed);
        fitsInside(squeezed, 85f, title, hint, "squeezed");
        Plan tiny = VoiceListeningPolicy.plan(50f, title, hint, 1f);
        check(near(tiny.halo(), 0f) && tiny.orb() < VoiceListeningPolicy.ORB_MIN_DP, "a tiny panel shrinks the orb: " + tiny);
        fitsInside(tiny, 50f, title, hint, "tiny");
        Plan none = VoiceListeningPolicy.plan(-5f, title, hint, 1f);
        check(none.orb() == 0f && none.halo() == 0f, "a collapsed panel draws no orb");

        // 大字号：同样高度下文字行更高，圆盘让路但仍在面板里。
        Plan large = VoiceListeningPolicy.plan(142f, 32f, 26f, 1f);
        check(large.orb() <= phone.orb(), "larger text never grows the orb");
        fitsInside(large, 142f, 32f, 26f, "large text");

        // 像素密度只是等比放大。
        Plan dense = VoiceListeningPolicy.plan(142f * 3.5f, title * 3.5f, hint * 3.5f, 3.5f);
        check(near(dense.orb(), phone.orb() * 3.5f) && dense.titleLines() == phone.titleLines(), "density scales the plan");

        // 放得下一行标题的每个高度都不越界；提示行一旦放得下，更高的面板不会再把它藏掉（藏提示是为了给圆盘让位，圆盘因此会在那一下变小）。
        boolean hintShown = false;
        for (int height = 39; height <= 400; height++) {
            Plan plan = VoiceListeningPolicy.plan(height, title, hint, 1f);
            fitsInside(plan, height, title, hint, "height " + height);
            check(plan.showsHint() || !hintShown, "the hint stays once it fits, at " + height);
            hintShown |= plan.showsHint();
            check(plan.orb() <= 72f && plan.halo() <= plan.orb() * VoiceListeningPolicy.HALO_RATIO + 1e-3f,
                "orb and halo stay in range at " + height);
        }

        // 音量平滑：变大快、变小慢，NaN 当作安静，单帧推进有上限。
        float attack = VoiceListeningPolicy.step(0f, 1f, 16f);
        float release = 1f - VoiceListeningPolicy.step(1f, 0f, 16f);
        check(attack > release * 3f, "rising follows faster than falling: " + attack + " / " + release);
        check(VoiceListeningPolicy.step(.5f, Float.NaN, 16f) < .5f, "NaN counts as silence");
        check(VoiceListeningPolicy.step(0f, 5f, 1000f) == VoiceListeningPolicy.step(0f, 1f, 100f), "targets clamp and steps cap at 100ms");
        check(VoiceListeningPolicy.step(.3f, .9f, 0f) == .3f && VoiceListeningPolicy.step(.3f, .9f, -5f) == .3f,
            "no time, no change");
        check(VoiceListeningPolicy.step(.995f, 1f, 16f) == 1f, "a value next to the target snaps to it");
        float shown = 0f;
        int frames = 0;
        while (!VoiceListeningPolicy.settled(shown, 1f) && frames < 100) {
            shown = VoiceListeningPolicy.step(shown, 1f, 16f);
            frames++;
        }
        check(frames < 30 && shown == 1f, "attack settles within half a second: " + frames);
        frames = 0;
        while (!VoiceListeningPolicy.settled(shown, 0f) && frames < 200) {
            shown = VoiceListeningPolicy.step(shown, 0f, 16f);
            frames++;
        }
        check(frames > 30 && frames < 120 && shown == 0f, "release decays over a second or two: " + frames);
        check(near(VoiceListeningPolicy.spread(0f, 18f), 18f * VoiceListeningPolicy.IDLE)
            && near(VoiceListeningPolicy.spread(1f, 18f), 18f) && near(VoiceListeningPolicy.spread(7f, 18f), 18f),
            "the halo idles at 30% and never passes its reserved width");

        // 中间文字：规整空白，放得下原样，放不下从开头省略并保住最新的字。
        check(VoiceListeningPolicy.normalized("  今天\n\t天气　不错  ").equals("今天 天气 不错"), "whitespace collapses");
        check(VoiceListeningPolicy.normalized(null).isEmpty(), "null is empty");
        check(VoiceListeningPolicy.fitTail("今天天气不错", text -> text.length() <= 10).equals("今天天气不错"), "short text stays");
        String fitted = VoiceListeningPolicy.fitTail("我们明天上午十点在公司门口集合然后一起出发", text -> text.length() <= 8);
        check(fitted.equals("…合然后一起出发"), "the ellipsis leads and the longest fitting tail survives: " + fitted);
        String emoji = VoiceListeningPolicy.fitTail("开会😀😀😀结束", text -> text.codePointCount(0, text.length()) <= 4);
        check(emoji.equals("…😀结束"), "surrogate pairs stay whole: " + emoji);
        check(VoiceListeningPolicy.fitTail("长长的一句话", text -> false).equals("…话"), "nothing fits keeps the last character");
        check(VoiceListeningPolicy.fitTail("   ", text -> false).isEmpty(), "blank stays empty");
        System.out.println("VoiceListeningPolicySmoke passed");
    }
}
