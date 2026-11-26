package app.msime.android;

/**
 * 键面随打字、切层变化的那几项（与之相对，{@link KeyFaceLayout} 是一副键盘显示期间不变的）：一个键此刻处在哪个状态由它们和键盘布局决定（{@link KeyFaceSpec#state}）。都是服务里已经算好的值，由 {@code ImeKeyFaces} 读出来，这里不读服务。
 *
 * @param composing 引擎正在组字（日语是编辑中的文字不为空）
 * @param symbols 显示的是 123 / 符号层，不是字母层
 * @param english 英文模式（{@code dedicatedEnglish}）
 * @param chineseSession 有会话、有视图、不是英文、不是密码框：可以按中文方式处理按键
 * @param localModeNone 没有临时模式（{@code local_mode} 为 {@code none}）
 * @param capsLock 字母大小写处于锁定大写
 * @param helpcode 14/18 键此刻能用网格辅助码（视图的 {@code grid_helpcode_available}）
 * @param engineScheme 引擎方案号（{@link InputSchemeTraits}），没有视图时为 -1
 * @param selected 选中的触屏方案；没有时为 {@code null}
 */
public record KeyFaceLive(boolean composing, boolean symbols, boolean english, boolean chineseSession,
        boolean localModeNone, boolean capsLock, boolean helpcode, int engineScheme, KeyboardScheme selected) {
}
