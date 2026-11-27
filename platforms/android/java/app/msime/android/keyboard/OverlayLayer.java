package app.msime.android;

/**
 * 键盘面板和画在它上面的各层，按从下到上的顺序列出。后面的层总画在前面的层之上，不用数字 z 值。
 *
 * <p>每一层放在一种窗口里（{@link Host}）。规则：一层只能放在不低于它下面各层的窗口里，否则下面那层所在的窗口会盖住它。同一个窗口里的层按这里的顺序做子视图，前面的先加；放在弹窗里的层各自是一个子窗口，子窗口总在输入法窗口之上。
 */
public enum OverlayLayer {
    /** 键盘面板本身（{@code keyboardSurface}）。 */
    PANEL(Host.IME_WINDOW),
    /** 浮在面板上沿之上的读音条。还没有用到，先占住它在顺序里的位置。 */
    READING_STRIP(Host.IME_WINDOW),
    /** 按键气泡和上滑提示，和按键同一帧画出。 */
    KEY_PREVIEW(Host.IME_WINDOW),
    /** 长按选项。它是自己的 PopupWindow：松手后点选由它接收，按住拖选时手势仍在键盘窗口里。 */
    LONG_PRESS(Host.POPUP_WINDOW),
    /** 文本编辑面板的放大镜，也是自己的 PopupWindow，和长按选项不会同时出现。 */
    MAGNIFIER(Host.POPUP_WINDOW);

    /** 放层的窗口，从下到上。 */
    public enum Host {
        /** 输入法窗口：层是 {@code keyboardRoot} 里排在面板之后的子视图。 */
        IME_WINDOW,
        /** 输入法窗口的子窗口（PopupWindow）。 */
        POPUP_WINDOW
    }

    public final Host host;

    OverlayLayer(Host host) {
        this.host = host;
    }

    /** 规则成立：从下往上，每一层的窗口都不低于它下面那一层的窗口。 */
    public static boolean ordered() {
        OverlayLayer[] layers = values();
        for (int index = 1; index < layers.length; index++)
            if (layers[index].host.compareTo(layers[index - 1].host) < 0) return false;
        return true;
    }
}
