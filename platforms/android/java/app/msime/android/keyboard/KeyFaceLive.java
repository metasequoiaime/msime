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
 * @param numberPad 画的是数字九宫格
 * @param sharedSymbolRows 画的是注音（大千、注音九键）共用的 #+= 符号行
 * @param chineseFace 中文键面（{@link KeyboardLayout#chineseFace}）：借来的数字九宫格上返回键写「返回」
 * @param japanese 日语方案在用（回车组字时写「確定」）
 * @param returnConfirms 回车此刻是「确认」：有组字、且不是字母组合方案在拼（{@code returnKeyConfirms}）
 * @param translationHold 长按回车能上屏译文（角标「译」）
 * @param newlineHold 长按回车能换行（角标「换行」）
 */
public record KeyFaceLive(boolean composing, boolean symbols, boolean english, boolean chineseSession,
        boolean localModeNone, boolean capsLock, boolean helpcode, int engineScheme, KeyboardScheme selected,
        boolean numberPad, boolean sharedSymbolRows, boolean chineseFace, boolean japanese, boolean returnConfirms,
        boolean translationHold, boolean newlineHold) {
}
