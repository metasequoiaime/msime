import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;

/** 设置页的手势说明必须解释方向、开关关系和九键滑动距离的含义。 */
public final class KeyboardGestureDescriptionSmoke {
    public static void main(String[] args) throws Exception {
        Path source = repoRoot().resolve("platforms/android/java/app/msime/android/home/KeyboardOptionsPage.java");
        String text = Files.readString(source, StandardCharsets.UTF_8);
        check(text.contains("GroupCard.Row directionRow = gestures.nav(\"滑动方向\", \"选择在字母键上滑动输入角标符号的方向；关闭滑动输入符号后不生效\""),
            "滑动方向必须说明方向和总开关关系");
        check(text.contains("gestures.slider(\"九键滑动距离\",\n                \"手指移动超过这个距离才会触发；距离越大越不容易把快速连打误判成滑动\""),
            "九键滑动距离必须说明阈值和误触影响");
        Path expression = repoRoot().resolve("platforms/android/java/app/msime/android/home/ExpressionPage.java");
        String expressionText = Files.readString(expression, StandardCharsets.UTF_8);
        check(expressionText.contains("QUOTE_STYLE_LABELS, quote), true);"),
            "引号样式的长说明和值必须纵向排列");
        System.out.println("Keyboard gesture descriptions passed");
    }

    private static Path repoRoot() {
        Path path = Paths.get(System.getProperty("user.dir")).toAbsolutePath();
        while (path != null) {
            if (Files.isRegularFile(path.resolve("platforms/android/java/app/msime/android/home/KeyboardOptionsPage.java"))) return path;
            path = path.getParent();
        }
        throw new IllegalStateException("repository root not found");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
