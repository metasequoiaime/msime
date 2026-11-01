package app.msime.android;

/** 没读到实时偏好时，建会话用的偏好副本按已记住的触屏键面起哪种布局。 */
public final class TouchLayoutHintPolicy {
    private TouchLayoutHintPolicy() {}

    /**
     * 返回建会话时偏好副本里应写的 `touch_keyboard_layout`。
     *
     * <p>runtime-options.json 里的偏好副本是首次安装时的出厂默认（全拼 26 键）。实时偏好没读到时拿它建会话，引擎是 26 键，九键用户的键盘就会先跳回 26 键。上次确认的键面是全拼九键、副本又是全拼 26 键时，把副本换成九键，引擎和画出的键面从同一个九键起来；实时偏好随后到达时照常换成用户的真实设置。只改这一种组合：键面永远只按引擎的视图画，不能画一套引擎没有开的网格，否则网格键送出的数字在 26 键会话里是废键。
     *
     * @param remembered 上次引擎视图给出的触屏布局（{@link KeyboardLayout} 的常量）
     * @param scheme 副本里的 `scheme`
     * @param layout 副本里的 `touch_keyboard_layout`
     */
    public static String startupTouchKeyboardLayout(int remembered, String scheme, String layout) {
        if (remembered == KeyboardLayout.QUANPIN_NINE_KEY_LAYOUT && "quanpin".equals(scheme)
                && "twenty_six_key".equals(layout)) return "nine_key";
        return layout;
    }
}
