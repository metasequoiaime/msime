package app.msime.android;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.EnumSet;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * 空格键上的麦克风跟着「长按空格语音输入」走（#10143）：关掉后长按空格不进语音，键面也不能再画麦克风。
 *
 * <p>{@link SpaceKeyFace} 是 View，JVM 冒烟里只有抛 `Stub!` 的桩，画不出来；这里按源码钉住 {@link ImeBottomRow} 给主空格和分离键盘右半边的空格都按同一个开关设置麦克风。源码比对前先把连续空白压成一个空格，换行、缩进的改动不会让它失败。
 *
 * <p>数字页的空格（#10059）：主标画 ␣，麦克风只在右上角作次要提示、同样跟着长按空格语音开关，不再只画一个麦克风让人当成点按语音键；读屏名称仍以「空格」开头。按键本身的点按、长按语音和拖动移光标不变，这里不碰。
 *
 * <p>语音页「无」关闭两个快捷入口，保留「更多工具」的语音磁贴；工具栏语音开启时也保留磁贴。这里核对模型入口和面板接线，组合状态由设备专项覆盖。
 */
public final class SpaceKeyMicSmoke {
    public static void main(String[] args) throws Exception {
        // 数字页空格的主标与 ⌫ 等图标键一样大，右上角的麦克风明显更小，只是提示。
        check(SpaceKeyFace.NUMBER_PAD_GLYPH_DP == KeyboardIconKey.ICON_DP, "the number pad space glyph is as large as the backspace icon");
        check(SpaceKeyFace.NUMBER_PAD_MIC_DP < SpaceKeyFace.NUMBER_PAD_GLYPH_DP && SpaceKeyFace.NUMBER_PAD_MIC_DP < SpaceKeyFace.MIC_DP,
            "the number pad mic is a smaller secondary hint");
        check(KeyboardIconPaths.Icon.SPACE_BAR.stroked(), "the space glyph is a stroked icon");

        String bottomRow = read("core/ImeBottomRow.java");
        check(body(bottomRow, "private boolean spaceShowsMic() {")
                .contains("return touchPreference(AndroidLocalSettings.SPACE_VOICE);"),
            "the space mic follows the long-press voice switch");
        String update = body(bottomRow, "void updateActionRow() {");
        check(update.contains("face.setShowsMic(spaceShowsMic());"),
            "the main space key sets its mic on every action-row refresh");
        check(update.contains("face.setNumberPadFace(numberPad);")
                && update.contains("face.setSchemeLabel(numberPad ? \"\" : spaceLabel());"),
            "the number pad space switches to the glyph face and back on every action-row refresh");
        String face = read("keyboard/SpaceKeyFace.java");
        check(body(face, "protected void onDraw(Canvas canvas) {")
                .contains("if (numberPadFace && transientLabel.isEmpty()) { drawNumberPadFace(canvas); return; }"),
            "the number pad face replaces the mic-and-label face unless a gesture overlay is showing");
        String numberPadFace = body(face, "private void drawNumberPadFace(Canvas canvas) {");
        int glyph = numberPadFace.indexOf("KeyboardIconPaths.Icon.SPACE_BAR");
        int guard = numberPadFace.indexOf("if (!showsMic) return;");
        int mic = numberPadFace.indexOf("KeyboardIconPaths.Icon.MIC");
        check(glyph >= 0 && guard > glyph && mic > guard,
            "the number pad face always draws the space glyph and the mic only behind the long-press voice switch");
        // 角标麦克风与九键角标数字同色同透明度：取皮肤 hint，停用时和 ␣ 一起调暗。
        check(numberPadFace.contains("ColorPolicy.withAlpha(cornerHintColor, ColorPolicy.enabledAlpha(isEnabled(), 255, 96))"),
            "the corner mic uses the corner hint colour and dims when the key is disabled");
        check(read("core/ImeStyler.java").contains("space.setCornerHintColor(color(target.hint()));"),
            "the styler gives the space key the skin's corner hint colour");
        check(read("core/MSIMEInputService.java").contains("SPACE_CURSOR_DESCRIPTION = \"空格；"),
            "screen readers still announce the number pad key as 空格");
        check(body(bottomRow, "void syncSplitSpace() {").contains("splitSpace.setShowsMic(spaceShowsMic());"),
            "the split keyboard's right-hand space mirrors the mic");

        // 「更多工具」里的语音磁贴不随工具栏开关隐藏。
        FunctionPanelModel.Item voiceTile = FunctionPanelModel.item(FunctionPanelModel.Id.VOICE_INPUT);
        check(FunctionPanelModel.itemsExcept(EnumSet.noneOf(FunctionPanelModel.Id.class)).contains(voiceTile),
            "the more-tools panel lists a voice tile");
        String panel = read("core/ImeFunctionPanel.java");
        check(!panel.contains("hiddenItems.add(FunctionPanelModel.Id.VOICE_INPUT)"),
            "the voice tile stays available in every shortcut state");
        check(read("keyboard/FunctionPanelView.java").contains("setContentDescription(\"更多工具\");"),
            "the panel is called 更多工具");

        String voicePage = read("home/VoicePage.java");
        check(voicePage.contains("new OptionSheet(requireContext(), \"启动方式\", TRIGGER_SHEET_HINT)"),
            "the trigger sheet explains 无");
        // 说明收进「?」（GroupCard.Row#help）；工具栏隐藏的提醒是状态，留在副标题。
        check(voicePage.contains("String triggerHint = trigger == TRIGGER_NONE ? TRIGGER_NONE_HINT")
                && voicePage.contains("recognition.nav(\"启动方式\", triggerWarning,")
                && voicePage.contains("() -> pickTrigger(trigger, triggerRow[0])).help(triggerHint);"),
            "the trigger row explains 无 once chosen");
        Matcher hintMatch = Pattern.compile("static final String TRIGGER_NONE_HINT = \"([^\"]*)\";").matcher(voicePage);
        check(hintMatch.find(), "missing TRIGGER_NONE_HINT");
        String hint = hintMatch.group(1);
        check(hint.contains("长按空格只输入空格"), "the 无 hint says what a long press does now");
        check(hint.contains("「更多工具」") && hint.contains("「" + voiceTile.label() + "」"),
            "the 无 hint points at the voice tile that stays in 更多工具");
        System.out.println("SpaceKeyMicSmoke passed");
    }

    /** `signature` 起到它的第一个方法体结束为止的一段源码（已压空白），只认 `{` `}` 配对，不处理字符串里的括号。 */
    private static String body(String source, String signature) {
        int start = source.indexOf(signature);
        check(start >= 0, "missing " + signature);
        int depth = 0;
        for (int index = start; index < source.length(); index++) {
            char c = source.charAt(index);
            if (c == '{') depth++;
            else if (c == '}' && --depth == 0) return source.substring(start, index + 1);
        }
        throw new AssertionError("unterminated " + signature);
    }

    /** 读源码并把连续空白压成一个空格。 */
    private static String read(String relative) throws Exception {
        return squash(Files.readString(repoRoot().resolve("platforms/android/java/app/msime/android/" + relative),
            StandardCharsets.UTF_8));
    }

    private static String squash(String source) {
        return source.replaceAll("\\s+", " ");
    }

    private static Path repoRoot() {
        Path path = Paths.get(System.getProperty("user.dir")).toAbsolutePath();
        while (path != null) {
            if (Files.isRegularFile(path.resolve("platforms/android/java/app/msime/android/core/ImeBottomRow.java"))) return path;
            path = path.getParent();
        }
        throw new IllegalStateException("repository root not found");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
