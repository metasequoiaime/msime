package app.msime.android;

import java.util.ArrayList;
import java.util.List;

/**
 * 按键区的按键音和振动在手指按下时给出，抬手上屏时不再给第二次（#10757）：宿主这一侧不依赖 Android 视图的部分，便于在 JVM 上测试。
 *
 * <p>按键的动作挂在点击上，点击要等手指抬起才发生；按键反馈以前也挂在点击上，所以打字有「不跟手」的延迟。现在按键区（{@link KeyboardKeyArea}）在手指按下、命中一个键时就给反馈，并在两段时间里武装（{@link #arm}）这个键：一段是把按下交给键的那一次分发（退格没有撤回备份时就在这里点击自己），另一段是把抬起交给键的那一次分发，连同框架随后投递的点击。这两段里这个键的点击反馈被消耗掉（{@link #consume}），只计数、播动画，不再出声、不再振动；两段各自结束后由 {@link #settle} 撤回没被消耗的武装。
 *
 * <p>按住期间的确认性反馈（长按弹出选项、连删、上滑清空待命、九键反方向滑出）落在两段之外，照常出声振动。抬手时的确认（上滑清空松手执行、松手才补判出的长按语音）由调用方绕开消耗，见 {@code ImeKeyFeedback#confirm}。
 *
 * <p>武装按令牌记，不按次数：同一个键前后两次武装的撤回各撤各的，一次被点击消耗过的武装，它自己的撤回不会误撤另一次。
 */
public final class KeyPressFeedbackPolicy<K> {
    /** 一次武装。键按身份比较。 */
    public static final class Token<K> {
        private final K key;

        private Token(K key) {
            this.key = key;
        }

        public K key() {
            return key;
        }
    }

    /** 还没被消耗、也没被撤回的武装，按武装的先后排列。 */
    private final List<Token<K>> armed = new ArrayList<>();

    /** 武装 `key`，直到返回的令牌被 {@link #settle} 撤回或被 {@link #consume} 消耗。`key` 为 null 时不武装，返回 null。 */
    public Token<K> arm(K key) {
        if (key == null) return null;
        Token<K> token = new Token<>(key);
        armed.add(token);
        return token;
    }

    /** 撤回一次武装（`token` 是 {@link #arm} 的返回值，宿主经视图回调原样交还，所以按 Object 收）；已经被消耗或已经撤回过的令牌、null 和别的对象什么也不做。 */
    public void settle(Object token) {
        if (token != null) armed.remove(token);
    }

    /** 点击路径询问 `key` 的反馈要不要出声振动：`key` 正被武装时消耗最早的那一次并返回 true（这次点击不再出声、不再振动）。 */
    public boolean consume(K key) {
        if (key == null) return false;
        for (int index = 0; index < armed.size(); index++) {
            if (armed.get(index).key == key) {
                armed.remove(index);
                return true;
            }
        }
        return false;
    }

    /** 还在武装中的次数，供测试和诊断。 */
    public int armedCount() {
        return armed.size();
    }
}
