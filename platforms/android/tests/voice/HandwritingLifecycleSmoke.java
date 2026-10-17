package app.msime.android;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;

/** 输入目标切换时，触屏手写面板自己的延迟任务也必须和服务会话一起失效。 */
public final class HandwritingLifecycleSmoke {
    public static void main(String[] args) throws Exception {
        Path root = Path.of(args.length == 0 ? "." : args[0]);
        String service = read(root.resolve("platforms/android/java/app/msime/android/core/MSIMEInputService.java"));
        String rows = read(root.resolve("platforms/android/java/app/msime/android/core/ImeLayoutRows.java"));
        check(rows.contains("void resetForInput()"),
            "ImeLayoutRows must expose an input-session reset boundary");
        check(rows.contains("cancelPendingInk();") && rows.contains("cancelHandwritingNotice();")
                && rows.contains("pinyinGeneration++"),
            "the reset must cancel delayed handwriting work and invalidate pinyin results");
        check(service.contains("imeLayoutRows.resetForInput();"),
            "the service must invalidate ImeLayoutRows work when the input session ends");
        System.out.println("Android handwriting lifecycle state is reset with the input session");
    }

    private static String read(Path path) throws Exception {
        return Files.readString(path, StandardCharsets.UTF_8);
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
