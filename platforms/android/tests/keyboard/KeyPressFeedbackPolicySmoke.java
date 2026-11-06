import app.msime.android.KeyPressFeedbackPolicy;
import app.msime.android.KeyPressFeedbackPolicy.Token;

/**
 * 按键区按下即反馈、抬手不再反馈（#10757）：按 {@link app.msime.android.KeyboardKeyArea} 调用的先后（按下前武装、按下分发后撤回；抬起前武装、抬起分发和随后投递的点击之后撤回）逐个场景模拟，检查每个场景里点击和确认性反馈各自会不会再出声振动。
 */
public final class KeyPressFeedbackPolicySmoke {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    public static void main(String[] args) {
        Object letter = new Object();
        Object other = new Object();

        // 普通字母键：按下给反馈并武装，按下分发里没有点击，撤回；抬起再武装，点击消耗它，之后的撤回什么也不撤。
        KeyPressFeedbackPolicy<Object> gate = new KeyPressFeedbackPolicy<>();
        Token<Object> down = gate.arm(letter);
        gate.settle(down);
        check(gate.armedCount() == 0, "the down window ends with the down dispatch");
        Token<Object> up = gate.arm(letter);
        check(gate.consume(letter), "the click of a key fed on press stays silent");
        gate.settle(up);
        check(gate.armedCount() == 0, "a consumed arm leaves nothing behind");
        check(!gate.consume(letter), "a click outside any window (an accessibility click) still sounds and vibrates");

        // 没有撤回备份的退格：按下分发里自己点击，消耗按下那次武装；之后按住连删落在两段之外，每一下照常反馈。
        down = gate.arm(letter);
        check(gate.consume(letter), "backspace clicking itself on press does not vibrate twice");
        gate.settle(down);
        check(!gate.consume(letter), "repeat deletes while held keep their feedback");
        up = gate.arm(letter);
        gate.settle(up);
        check(gate.armedCount() == 0, "a release with no click disarms the key");

        // 长按弹出选项：确认反馈在按下分发之后、抬起之前，不被消耗。
        down = gate.arm(letter);
        gate.settle(down);
        check(!gate.consume(letter), "a long-press confirmation still sounds and vibrates");

        // 两根手指：先按的键先抬，两次点击各消耗自己那次武装，另一个键不受影响。
        Token<Object> first = gate.arm(letter);
        gate.settle(first);
        Token<Object> second = gate.arm(other);
        gate.settle(second);
        Token<Object> firstUp = gate.arm(letter);
        check(!gate.consume(other), "the key still held is not armed by another finger's release");
        check(gate.consume(letter), "the released key's click is silent");
        gate.settle(firstUp);
        Token<Object> secondUp = gate.arm(other);
        check(gate.consume(other), "the second key's click is silent too");
        gate.settle(secondUp);
        check(gate.armedCount() == 0, "both fingers leave nothing armed");

        // 快速连点同一个键：上一次抬起的点击还排在后面时这一次已经按下。两次武装各自撤回，点击只消耗一次。
        Token<Object> previousUp = gate.arm(letter);
        Token<Object> nextDown = gate.arm(letter);
        gate.settle(nextDown);
        check(gate.consume(letter), "the late click of the previous tap is silent");
        gate.settle(previousUp);
        check(gate.armedCount() == 0, "a settle never removes another press's arm");
        check(!gate.consume(letter), "nothing stays armed after a fast double tap");

        // 撤回一个已经被消耗的令牌不会撤掉同一个键随后的另一次武装。
        Token<Object> consumed = gate.arm(letter);
        check(gate.consume(letter), "first arm consumed");
        Token<Object> later = gate.arm(letter);
        gate.settle(consumed);
        check(gate.armedCount() == 1, "settling a consumed token keeps the later arm");
        gate.settle(later);

        // 没有命中键时不武装。
        check(gate.arm(null) == null, "a press on no key arms nothing");
        gate.settle(null);
        check(!gate.consume(null), "a null source is never consumed");
        System.out.println("Android key press feedback: press-time feedback, silent release clicks and untouched confirmations passed");
    }
}
