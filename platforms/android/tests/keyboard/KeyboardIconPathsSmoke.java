import app.msime.android.KeyboardIconPaths;
import app.msime.android.KeyboardIconPaths.Icon;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.List;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

public final class KeyboardIconPathsSmoke {
    private static final String SCRIPT = "platforms/android/scripts/generate_keyboard_icons.py";
    private static final String OUTPUT = "platforms/android/java/app/msime/android/keyboard/KeyboardIconPaths.java";

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    // 从当前目录向上找仓库根（含生成脚本的那一层），冒烟可以在仓库内任意目录运行。
    private static Path repoRoot() {
        Path dir = Paths.get(System.getProperty("user.dir")).toAbsolutePath();
        while (dir != null) {
            if (Files.isRegularFile(dir.resolve(SCRIPT))) return dir;
            dir = dir.getParent();
        }
        throw new AssertionError("run this smoke from inside the repository; " + SCRIPT + " not found above user.dir");
    }

    public static void main(String[] args) throws IOException {
        Path root = repoRoot();
        String script = new String(Files.readAllBytes(root.resolve(SCRIPT)), StandardCharsets.UTF_8);
        String generated = new String(Files.readAllBytes(root.resolve(OUTPUT)), StandardCharsets.UTF_8);

        // 脚本 ICONS 表里的 (名字, 样式, 线宽) 必须与生成的枚举逐项一致、顺序相同。
        Matcher row = Pattern.compile("\\(\"([A-Z0-9_]+)\", (FILL|STROKE), ([0-9.]+),").matcher(script);
        List<String> names = new ArrayList<>();
        List<Boolean> strokes = new ArrayList<>();
        List<Float> widths = new ArrayList<>();
        while (row.find()) {
            names.add(row.group(1));
            strokes.add("STROKE".equals(row.group(2)));
            widths.add(Float.parseFloat(row.group(3)));
        }
        Icon[] icons = Icon.values();
        check(names.size() == icons.length, "script lists " + names.size() + " icons, Java has " + icons.length);
        for (int i = 0; i < icons.length; i++) {
            Icon icon = icons[i];
            check(icon.name().equals(names.get(i)), "icon " + i + " is " + icon.name() + " in Java, " + names.get(i) + " in script");
            check(icon.stroked() == strokes.get(i), "style of " + icon.name());
            float expectedWidth = strokes.get(i) ? widths.get(i) : 0f;
            check(Math.abs(icon.strokeWidth() - expectedWidth) < 1e-4f, "stroke width of " + icon.name());
        }

        // 工具栏六个图标、按键图标与对勾角标都在。
        for (String required : new String[] {"TOOLBAR_EMOJI", "TOOLBAR_PHRASE", "TOOLBAR_CLIPBOARD", "TOOLBAR_SKIN", "TOOLBAR_SCHEME", "TOOLBAR_DISMISS", "SHIFT", "CAPS_LOCK", "BACKSPACE", "RETURN", "MIC", "CHEVRON", "CHECK"}) {
            check(names.contains(required), "missing icon " + required);
        }
        for (String fill : new String[] {"TOOLBAR_EMOJI", "TOOLBAR_PHRASE", "TOOLBAR_CLIPBOARD", "TOOLBAR_SKIN", "TOOLBAR_SCHEME"}) {
            check(!Icon.valueOf(fill).stroked(), fill + " is a filled Material glyph");
        }
        check(KeyboardIconPaths.VIEWBOX == 24f, "viewBox 24");

        // 生成物头部注明来源与不得手工编辑，且不含 SVG 弧线命令的残留。
        check(generated.startsWith("// 由 platforms/android/scripts/generate_keyboard_icons.py 生成，不要手工编辑"), "generated header");
        check(!generated.contains("arcTo("), "arcs are converted to cubics in the script");
        for (Icon icon : icons) {
            String method = "append" + camel(icon.name()) + "(Path p)";
            check(generated.contains(method), "generated builder for " + icon.name());
        }
        System.out.println("Android keyboard icons: " + icons.length + " generated paths match the script");
    }

    private static String camel(String enumName) {
        StringBuilder out = new StringBuilder();
        for (String part : enumName.split("_")) {
            if (part.isEmpty()) continue;
            out.append(part.charAt(0)).append(part.substring(1).toLowerCase(java.util.Locale.ROOT));
        }
        return out.toString();
    }
}
