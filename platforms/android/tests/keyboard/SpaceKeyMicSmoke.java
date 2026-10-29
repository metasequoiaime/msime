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
 * <p>{@link SpaceKeyFace} 是 View，JVM 冒烟里只有抛 `Stub!` 的桩，画不出来；这里测纯函数 {@link SpaceKeyFace#numberPadLabel}，再按源码钉住 {@link ImeBottomRow} 给主空格和分离键盘右半边的空格都按同一个开关设置麦克风。源码比对前先把连续空白压成一个空格，换行、缩进的改动不会让它失败。
 *
 * <p>语音页「无」的说明要和键盘的真实行为对得上：选「无」时空格键和工具栏上没有语音入口，但「更多工具」面板的「语音输入」磁贴还在（只有工具栏语音按钮开着时才藏）。这里用 {@link FunctionPanelModel} 确认那块磁贴存在、钉住面板只在工具栏语音按钮的开关 `touchVoiceShortcutEnabled` 打开时才藏它（#10142 起看开关，不看按钮视图），并要求说明里写出磁贴的名字；哪天选「无」也藏掉这块磁贴，这里会失败，提醒同时改说明。
 */
public final class SpaceKeyMicSmoke {
    public static void main(String[] args) throws Exception {
        check(SpaceKeyFace.numberPadLabel(true).isEmpty(), "number pad space shows only the mic");
        check("空格".equals(SpaceKeyFace.numberPadLabel(false)), "number pad space without the mic is not blank");

        String bottomRow = read("core/ImeBottomRow.java");
        check(body(bottomRow, "private boolean spaceShowsMic() {")
                .contains("return touchPreference(AndroidLocalSettings.SPACE_VOICE);"),
            "the space mic follows the long-press voice switch");
        String update = body(bottomRow, "void updateActionRow() {");
        check(update.contains("face.setShowsMic(mic);")
                && update.contains("face.setSchemeLabel(numberPad ? SpaceKeyFace.numberPadLabel(mic) : spaceLabel());"),
            "the main space key sets its mic on every action-row refresh");
        check(body(bottomRow, "void syncSplitSpace() {").contains("splitSpace.setShowsMic(spaceShowsMic());"),
            "the split keyboard's right-hand space mirrors the mic");

        // 「更多工具」里的语音磁贴：模型里一直有它，面板只在工具栏语音按钮开着时才把它藏起来。
        FunctionPanelModel.Item voiceTile = FunctionPanelModel.item(FunctionPanelModel.Id.VOICE_INPUT);
        check(FunctionPanelModel.itemsExcept(EnumSet.noneOf(FunctionPanelModel.Id.class)).contains(voiceTile),
            "the more-tools panel lists a voice tile");
        String panel = read("core/ImeFunctionPanel.java");
        Matcher hides = Pattern.compile("hiddenItems\\.add\\(FunctionPanelModel\\.Id\\.VOICE_INPUT\\)").matcher(panel);
        int hidden = 0;
        while (hides.find()) hidden++;
        check(hidden == 1 && panel.contains(
                "if (s.touchVoiceShortcutEnabled) hiddenItems.add(FunctionPanelModel.Id.VOICE_INPUT);"),
            "the voice tile is hidden only while the toolbar voice button is switched on");
        check(read("keyboard/FunctionPanelView.java").contains("setContentDescription(\"更多工具\");"),
            "the panel is called 更多工具");

        String voicePage = read("home/VoicePage.java");
        check(voicePage.contains("new OptionSheet(requireContext(), \"启动方式\", TRIGGER_SHEET_HINT)"),
            "the trigger sheet explains 无");
        check(voicePage.contains("recognition.nav(\"启动方式\", trigger == TRIGGER_NONE ? TRIGGER_NONE_HINT : null,"),
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
