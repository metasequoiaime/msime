package app.msime.android;

/**
 * 长按选项弹窗的纵向位置（屏幕坐标，像素）。弹窗是输入法窗口的子窗口，画在键盘之上，可以伸出面板上沿，所以顶排键的选项照常浮在键的上方，不再压回键区盖住按键。
 */
public final class KeyOptionsPlacement {
    private KeyOptionsPlacement() { }

    /**
     * 弹窗的上沿：默认在键的上方隔一段间距；放不下时向下挪，但不高过屏幕可用区的上沿（状态栏和刘海之下），也不低到弹窗底边伸出键盘下沿。
     *
     * @param keyTop 键的上沿
     * @param height 弹窗的高度
     * @param gap 弹窗底边和键上沿之间的间距
     * @param safeTop 屏幕可用区的上沿
     * @param keyboardBottom 键盘根视图的下沿
     */
    public static int top(int keyTop, int height, int gap, int safeTop, int keyboardBottom) {
        return Math.max(safeTop, Math.min(keyTop - height - gap, keyboardBottom - height));
    }

    /**
     * 弹窗上沿最高能到哪里。API 29 及以下的子窗口被输入法父窗口裁剪：伸出窗口上沿的部分画不出来，所以那里还不高过输入法窗口的上沿（API 29 停靠时是面板上方透明高度的上沿，API 28 的窗口本来就是全高）。API 30 起子窗口能画到父窗口之外，只受屏幕可用区限制。
     *
     * @param screenSafeTop 屏幕可用区的上沿（状态栏、刘海之下）
     * @param imeWindowTop 输入法窗口的上沿（屏幕坐标）
     * @param sdk {@code Build.VERSION.SDK_INT}
     */
    public static int limit(int screenSafeTop, int imeWindowTop, int sdk) {
        return sdk <= 29 ? Math.max(screenSafeTop, imeWindowTop) : screenSafeTop;
    }
}
