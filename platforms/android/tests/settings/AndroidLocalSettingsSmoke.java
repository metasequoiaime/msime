package app.msime.android;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import java.util.stream.Stream;

/**
 * AndroidLocalSettings 不碰 org.json 的部分：默认值、取值校验、写入前的拒绝、读不到时回到默认值、原子写，以及参与同步的键与 client-core 的 `ANDROID_LOCAL_SETTINGS` 逐项一致（键名、类型、可选值、整数范围）。check-host 的 android.jar 里 org.json 只是抛 `Stub!` 的桩，编码与解码留给设备上的设置页与同步。
 */
public final class AndroidLocalSettingsSmoke {
    private static final String SYNC_SOURCE = "crates/client-core/src/account/settings_sync.rs";

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }

    private static Path repoRoot() {
        Path dir = Paths.get(System.getProperty("user.dir")).toAbsolutePath();
        while (dir != null) {
            if (Files.isRegularFile(dir.resolve(SYNC_SOURCE))) return dir;
            dir = dir.getParent();
        }
        throw new AssertionError("run this smoke from inside the repository; " + SYNC_SOURCE + " not found above user.dir");
    }

    public static void main(String[] args) throws IOException {
        defaults();
        validation();
        rejectedWritesTouchNothing();
        unreadableFilesReadAsDefaults();
        atomicWrite();
        splitKeyboardRoundTrip();
        syncedKeysMatchClientCore();
        System.out.println("AndroidLocalSettingsSmoke ok");
    }

    private static void defaults() {
        AndroidLocalSettings.Snapshot settings = AndroidLocalSettings.defaults();
        check("siji".equals(settings.choice(AndroidLocalSettings.APP_THEME)), "app theme default");
        check("off".equals(settings.choice(AndroidLocalSettings.ONE_HANDED)), "one-handed default");
        check(!settings.bool(AndroidLocalSettings.SPLIT_KEYBOARD), "split keyboard is off by default");
        check(settings.bool(AndroidLocalSettings.KEY_POPUP), "key popup default");
        check(settings.bool(AndroidLocalSettings.SWIPE_DOWN_SYMBOLS), "swipe default");
        check("down".equals(settings.choice(AndroidLocalSettings.SWIPE_SYMBOLS_DIRECTION)), "swipe direction default");
        check(settings.bool(AndroidLocalSettings.SPACE_CURSOR), "space cursor default");
        check(settings.bool(AndroidLocalSettings.SPACE_VOICE), "space voice default");
        check("none".equals(settings.choice(AndroidLocalSettings.KEY_ANIMATION)), "animation default");
        check(settings.bool(AndroidLocalSettings.TOOLBAR_PHRASE), "toolbar phrase default");
        check(settings.bool(AndroidLocalSettings.TOOLBAR_SCHEME), "toolbar scheme default");
        check(!settings.bool(AndroidLocalSettings.TOOLBAR_HIDDEN), "toolbar hidden default");
        check("overlap".equals(settings.choice(AndroidLocalSettings.HANDWRITING_MODE)), "handwriting mode default matches HandwritingPreferences");
        check(settings.integer(AndroidLocalSettings.HANDWRITING_DELAY_MS) == HandwritingPreferences.DELAY_DEFAULT, "handwriting delay default");
        check(settings.integer(AndroidLocalSettings.HANDWRITING_STROKE_WIDTH) == HandwritingPreferences.WIDTH_DEFAULT, "stroke width matches HandwritingPreferences");
        check(settings.bool(AndroidLocalSettings.HANDWRITING_SHOW_PINYIN), "show pinyin default");
        check("follow_skin".equals(settings.choice(AndroidLocalSettings.HANDWRITING_STROKE_COLOR)), "stroke colour default");
        check(settings.integer(AndroidLocalSettings.HANDWRITING_STROKE_WIDTH) == 3, "stroke width default");
        check(!settings.bool(AndroidLocalSettings.VOICE_OFFLINE_FALLBACK), "offline fallback default");
        check(!settings.bool(AndroidLocalSettings.INCOGNITO), "incognito default");
        check(!settings.bool(AndroidLocalSettings.VOICE_CONTRIBUTE_AUDIO), "contribute audio default");
        check(settings.integer(AndroidLocalSettings.KEYBOARD_HEIGHT_ADJUSTMENT) == 0, "height default");
        check(!settings.has(AndroidLocalSettings.KEYBOARD_HEIGHT_ADJUSTMENT), "height is not explicit by default");
        check(!settings.bool(AndroidLocalSettings.DEVELOPER_DEBUG_OVERLAY), "overlay default");
        check("warn".equals(settings.choice(AndroidLocalSettings.DEVELOPER_LOG_LEVEL)), "log level default");
        check(!settings.bool(AndroidLocalSettings.NUMBER_ROW), "number row is off by default");
        check(!AndroidLocalSettings.spec(AndroidLocalSettings.NUMBER_ROW).synced, "number row stays on this device");
        check(!settings.bool(AndroidLocalSettings.DEVELOPER_INPUT_LOG), "input log default");
        check("one_day".equals(settings.choice(AndroidLocalSettings.MCP_RETENTION)), "retention default");
        check(settings.bool(AndroidLocalSettings.MCP_CRASH_LOGS), "crash logs default");
        check(settings.bool(AndroidLocalSettings.MCP_PERFORMANCE_LOGS), "perf logs default");
        check(!settings.bool(AndroidLocalSettings.MCP_INPUT_EVENTS), "input events default off");
        check(settings.bool(AndroidLocalSettings.MCP_CONFIG_SNAPSHOT), "config snapshot default");
        check(settings.explicit().isEmpty(), "defaults carry no explicit values");
        // 每一项的默认值本身必须合规。
        for (AndroidLocalSettings.Spec spec : AndroidLocalSettings.specs().values()) {
            check(spec.defaultValue.equals(spec.accept(spec.defaultValue)), "default of " + spec.key + " is valid");
        }
        boolean threw = false;
        try { settings.bool(AndroidLocalSettings.APP_THEME); } catch (IllegalArgumentException expected) { threw = true; }
        check(threw, "reading a choice as a boolean is a programming error");
        threw = false;
        try { settings.bool("touch_incognito"); } catch (IllegalArgumentException expected) { threw = true; }
        check(threw, "the retired shared key is not a local setting");
    }

    private static void validation() {
        AndroidLocalSettings.Spec bool = AndroidLocalSettings.spec(AndroidLocalSettings.INCOGNITO);
        check(Boolean.TRUE.equals(bool.accept(true)), "boolean accepts true");
        check(bool.accept("true") == null, "boolean rejects text");
        check(bool.accept(1) == null, "boolean rejects numbers");

        AndroidLocalSettings.Spec choice = AndroidLocalSettings.spec(AndroidLocalSettings.ONE_HANDED);
        check("left".equals(choice.accept("left")), "choice accepts a listed value");
        check(choice.accept("Left") == null, "choice is exact");
        check(choice.accept("middle") == null, "choice rejects an unlisted value");

        AndroidLocalSettings.Spec delay = AndroidLocalSettings.spec(AndroidLocalSettings.HANDWRITING_DELAY_MS);
        check(Integer.valueOf(200).equals(delay.accept(200)), "delay lower bound");
        check(Integer.valueOf(1500).equals(delay.accept(1500L)), "delay upper bound as a long");
        check(Integer.valueOf(700).equals(delay.accept(700.0)), "a whole double is an integer");
        check(delay.accept(650) == null, "delay must be a multiple of 100 from 200");
        check(delay.accept(100) == null, "delay below range");
        check(delay.accept(1600) == null, "delay above range");
        check(delay.accept(700.5) == null, "fractions are rejected");
        check(delay.accept(Double.NaN) == null, "NaN is rejected");

        AndroidLocalSettings.Spec height = AndroidLocalSettings.spec(AndroidLocalSettings.KEYBOARD_HEIGHT_ADJUSTMENT);
        check(Integer.valueOf(-46).equals(height.accept(-46)), "height 75%");
        check(Integer.valueOf(55).equals(height.accept(55)), "height 130%");
        check(Integer.valueOf(110).equals(height.accept(110)), "height 160% (#5564)");
        check(height.accept(-47) == null && height.accept(111) == null, "height outside the design range");

        // 浮动键盘（#5621）：默认关，位置默认水平居中、贴底，千分比 0–1000，工具栏按钮默认不显示。
        AndroidLocalSettings.Snapshot fresh = AndroidLocalSettings.defaults();
        check(!fresh.bool(AndroidLocalSettings.FLOATING_KEYBOARD), "floating keyboard off by default");
        check(!fresh.bool(AndroidLocalSettings.TOOLBAR_FLOATING), "floating toolbar button hidden by default");
        // 工具栏的文本编辑按钮（#6351）：默认不显示，只在本机。
        check(!fresh.bool(AndroidLocalSettings.TOOLBAR_TEXT_EDIT), "text edit toolbar button hidden by default");
        check(fresh.integer(AndroidLocalSettings.FLOATING_KEYBOARD_X) == 500
            && fresh.integer(AndroidLocalSettings.FLOATING_KEYBOARD_Y) == 1000, "floating position default");
        // 键盘底栏：默认开，只在本机。
        check(fresh.bool(AndroidLocalSettings.BOTTOM_BAR), "bottom bar on by default");
        // 底部留白（#6392）：默认关，老用户的键盘高度不变；只在本机。
        check(!fresh.bool(AndroidLocalSettings.BOTTOM_PADDING), "bottom padding off by default");
        // 「加高底行」（#6354）已移除，底行默认就和键行同高：不再是一个设置项，存着的旧值读取时丢掉。
        check(!AndroidLocalSettings.specs().containsKey("platform.android.tall_bottom_row"), "tall bottom row is no longer a setting");
        AndroidLocalSettings.Spec floatingX = AndroidLocalSettings.spec(AndroidLocalSettings.FLOATING_KEYBOARD_X);
        check(Integer.valueOf(0).equals(floatingX.accept(0)) && Integer.valueOf(1000).equals(floatingX.accept(1000))
            && floatingX.accept(-1) == null && floatingX.accept(1001) == null, "floating position range");

        Map<String, Object> raw = new LinkedHashMap<>();
        raw.put(AndroidLocalSettings.APP_THEME, "dongxue");
        raw.put(AndroidLocalSettings.ONE_HANDED, "middle");
        raw.put(AndroidLocalSettings.HANDWRITING_STROKE_WIDTH, 5L);
        raw.put("platform.android.future_setting", true);
        // 本机或备份里留着的已移除设置（#6354 的「加高底行」）和不认识的键一样丢掉，不让整份读取失败。
        raw.put("platform.android.tall_bottom_row", true);
        Map<String, Object> accepted = AndroidLocalSettings.accepted(raw);
        check(accepted.size() == 2, "unknown and invalid keys are dropped: " + accepted);
        check("dongxue".equals(accepted.get(AndroidLocalSettings.APP_THEME)), "valid choice kept");
        check(Integer.valueOf(5).equals(accepted.get(AndroidLocalSettings.HANDWRITING_STROKE_WIDTH)), "long normalised to integer");

        AndroidLocalSettings.Snapshot snapshot = new AndroidLocalSettings.Snapshot(accepted);
        check("dongxue".equals(snapshot.choice(AndroidLocalSettings.APP_THEME)), "explicit value wins");
        check("off".equals(snapshot.choice(AndroidLocalSettings.ONE_HANDED)), "dropped value reads its default");
        check(snapshot.has(AndroidLocalSettings.APP_THEME) && !snapshot.has(AndroidLocalSettings.ONE_HANDED), "has()");
        Map<String, Object> synced = snapshot.synced();
        check(synced.size() == 18, "eighteen synced keys, got " + synced.size());
        check(Boolean.FALSE.equals(synced.get(AndroidLocalSettings.SPLIT_KEYBOARD)), "split keyboard syncs its default");
        check("dongxue".equals(synced.get(AndroidLocalSettings.APP_THEME)), "synced carries explicit values");
        check(Boolean.TRUE.equals(synced.get(AndroidLocalSettings.KEY_POPUP)), "synced carries defaults");
        for (String local : new String[] {AndroidLocalSettings.INCOGNITO, AndroidLocalSettings.VOICE_CONTRIBUTE_AUDIO,
                AndroidLocalSettings.KEYBOARD_HEIGHT_ADJUSTMENT, AndroidLocalSettings.FLOATING_KEYBOARD,
                AndroidLocalSettings.FLOATING_KEYBOARD_X, AndroidLocalSettings.FLOATING_KEYBOARD_Y,
                AndroidLocalSettings.TOOLBAR_FLOATING, AndroidLocalSettings.TOOLBAR_TEXT_EDIT, AndroidLocalSettings.BOTTOM_BAR, AndroidLocalSettings.BOTTOM_PADDING,
                AndroidLocalSettings.DEVELOPER_DEBUG_OVERLAY,
                AndroidLocalSettings.DEVELOPER_LOG_LEVEL, AndroidLocalSettings.DEVELOPER_INPUT_LOG,
                AndroidLocalSettings.MCP_RETENTION, AndroidLocalSettings.MCP_INPUT_EVENTS}) {
            check(!synced.containsKey(local), local + " never syncs");
        }
        check(snapshot.equals(new AndroidLocalSettings.Snapshot(new TreeMap<>(accepted))), "snapshots compare by value");
    }

    private static void rejectedWritesTouchNothing() throws IOException {
        Path directory = Files.createTempDirectory("android-local-settings");
        try {
            Path file = directory.resolve("state").resolve(AndroidLocalSettings.FILE_NAME);
            Map<String, Object> edits = new LinkedHashMap<>();
            edits.put(AndroidLocalSettings.INCOGNITO, "yes");
            boolean threw = false;
            try { AndroidLocalSettings.update(file, edits); } catch (IllegalArgumentException expected) { threw = true; }
            check(threw, "an invalid value is refused");
            edits.clear();
            edits.put("touch_one_handed", "left");
            threw = false;
            try { AndroidLocalSettings.update(file, edits); } catch (IllegalArgumentException expected) { threw = true; }
            check(threw, "an unknown key is refused");
            check(!Files.exists(file.getParent()), "a refused write creates nothing");

            Map<String, Object> cloud = new LinkedHashMap<>();
            cloud.put(AndroidLocalSettings.INCOGNITO, true);
            cloud.put(AndroidLocalSettings.ONE_HANDED, "middle");
            cloud.put("platform.android.key_sound_pack", "typewriter");
            check(AndroidLocalSettings.applySynced(file, cloud).explicit().isEmpty(),
                "local-only, invalid and foreign keys from the cloud are ignored");
            check(!Files.exists(file), "nothing to apply leaves the file alone");
        } finally {
            deleteTree(directory);
        }
    }

    private static void unreadableFilesReadAsDefaults() throws IOException {
        Path directory = Files.createTempDirectory("android-local-settings");
        try {
            Path file = directory.resolve(AndroidLocalSettings.FILE_NAME);
            check(AndroidLocalSettings.load(file).equals(AndroidLocalSettings.defaults()), "missing file reads as defaults");
            byte[] large = new byte[AndroidLocalSettings.MAX_BYTES + 1];
            Arrays.fill(large, (byte) ' ');
            Files.write(file, large);
            check(AndroidLocalSettings.load(file).equals(AndroidLocalSettings.defaults()), "oversized file reads as defaults");
            check(Files.size(file) == large.length, "reading never rewrites the file");
            Files.delete(file);
            Files.createDirectory(file);
            check(AndroidLocalSettings.load(file).equals(AndroidLocalSettings.defaults()), "a directory reads as defaults");
            Files.delete(file);
            Path outside = directory.resolve("outside-settings.json");
            Files.writeString(outside, "synthetic");
            Files.createLink(file, outside);
            check(!AndroidLocalSettings.privateFile(file), "a hard-linked settings file is refused");
        } finally {
            deleteTree(directory);
        }
    }

    private static void atomicWrite() throws IOException {
        Path directory = Files.createTempDirectory("android-local-settings");
        try {
            Path file = directory.resolve(AndroidLocalSettings.FILE_NAME);
            Files.write(file, "old".getBytes(StandardCharsets.UTF_8));
            AndroidLocalSettings.writeAtomically(file, "new".getBytes(StandardCharsets.UTF_8));
            check("new".equals(new String(Files.readAllBytes(file), StandardCharsets.UTF_8)), "content replaced");
            try (Stream<Path> entries = Files.list(directory)) {
                check(entries.count() == 1, "no temporary file is left behind");
            }
            boolean threw = false;
            try {
                AndroidLocalSettings.writeAtomically(file, new byte[AndroidLocalSettings.MAX_BYTES + 1]);
            } catch (IOException expected) {
                threw = true;
            }
            check(threw, "an oversized document is never written");
            check("new".equals(new String(Files.readAllBytes(file), StandardCharsets.UTF_8)), "refused write keeps the old file");
        } finally {
            deleteTree(directory);
        }
    }

    /** 分离式键盘开关：只收布尔值，写进去的值读得回来，删掉后回到默认的关；云端同步来的值照常写回。 */
    private static void splitKeyboardRoundTrip() throws IOException {
        AndroidLocalSettings.Spec spec = AndroidLocalSettings.spec(AndroidLocalSettings.SPLIT_KEYBOARD);
        check(spec.kind == AndroidLocalSettings.Spec.Kind.BOOLEAN && spec.synced, "split keyboard is a synced boolean");
        check(spec.accept("true") == null && spec.accept(1) == null, "split keyboard only accepts booleans");
        Path directory = Files.createTempDirectory("android-local-settings");
        try {
            Path file = directory.resolve("state").resolve(AndroidLocalSettings.FILE_NAME);
            Map<String, Object> edits = new LinkedHashMap<>();
            edits.put(AndroidLocalSettings.SPLIT_KEYBOARD, "on");
            boolean threw = false;
            try { AndroidLocalSettings.update(file, edits); } catch (IllegalArgumentException expected) { threw = true; }
            check(threw, "a non-boolean split keyboard value is refused");
            Map<String, Object> cloud = new LinkedHashMap<>();
            cloud.put(AndroidLocalSettings.SPLIT_KEYBOARD, "yes");
            check(AndroidLocalSettings.applySynced(file, cloud).explicit().isEmpty(), "an invalid synced value is ignored");
            check(!Files.exists(file), "ignoring it leaves the file alone");
            // 写入走 org.json 编码，check-host 的 android.jar 里只有桩，所以这里只核对写入前的校验和快照；真正落盘的往返由设备上的设置页覆盖。
            AndroidLocalSettings.Snapshot on = new AndroidLocalSettings.Snapshot(
                AndroidLocalSettings.accepted(Map.of(AndroidLocalSettings.SPLIT_KEYBOARD, true)));
            check(on.bool(AndroidLocalSettings.SPLIT_KEYBOARD) && on.has(AndroidLocalSettings.SPLIT_KEYBOARD), "an explicit on reads back");
            check(Boolean.TRUE.equals(on.synced().get(AndroidLocalSettings.SPLIT_KEYBOARD)), "an explicit on syncs");
            AndroidLocalSettings.Snapshot cleared = new AndroidLocalSettings.Snapshot(
                AndroidLocalSettings.accepted(Map.of(AndroidLocalSettings.SPLIT_KEYBOARD, "true")));
            check(!cleared.bool(AndroidLocalSettings.SPLIT_KEYBOARD) && !cleared.has(AndroidLocalSettings.SPLIT_KEYBOARD), "a stored non-boolean reads as off");
        } finally {
            deleteTree(directory);
        }
    }

    /** client-core 校验 `android_local` 用的表与这里的同步键逐项一致。 */
    private static void syncedKeysMatchClientCore() throws IOException {
        String source = new String(Files.readAllBytes(repoRoot().resolve(SYNC_SOURCE)), StandardCharsets.UTF_8);
        int start = source.indexOf("const ANDROID_LOCAL_SETTINGS");
        check(start >= 0, "ANDROID_LOCAL_SETTINGS not found in " + SYNC_SOURCE);
        int end = source.indexOf("\n];", start);
        String table = source.substring(start, end);
        Matcher ids = Pattern.compile("const APP_THEME_IDS: \\[&str; \\d+\\] = \\[([^\\]]*)\\];").matcher(source);
        check(ids.find(), "APP_THEME_IDS not found");
        table = table.replace("&APP_THEME_IDS", "&[" + ids.group(1) + "]");
        Matcher entry = Pattern.compile(
            "\"([a-z_.]+)\",\\s*LocalSetting::(Boolean|Choice\\(&\\[([^\\]]*)\\]\\)|Integer \\{([^}]*)\\})",
            Pattern.DOTALL).matcher(table);
        List<String> rustKeys = new ArrayList<>();
        while (entry.find()) {
            String key = entry.group(1);
            rustKeys.add(key);
            AndroidLocalSettings.Spec spec = AndroidLocalSettings.spec(key);
            check(spec.synced, key + " syncs on the Rust side but not here");
            if (entry.group(2).equals("Boolean")) {
                check(spec.kind == AndroidLocalSettings.Spec.Kind.BOOLEAN, key + " kind");
            } else if (entry.group(3) != null) {
                check(spec.kind == AndroidLocalSettings.Spec.Kind.CHOICE, key + " kind");
                List<String> choices = new ArrayList<>();
                Matcher quoted = Pattern.compile("\"([a-z_0-9]+)\"").matcher(entry.group(3));
                while (quoted.find()) choices.add(quoted.group(1));
                check(choices.equals(Arrays.asList(spec.choices())), key + " choices " + choices + " vs " + Arrays.asList(spec.choices()));
            } else {
                check(spec.kind == AndroidLocalSettings.Spec.Kind.INTEGER, key + " kind");
                String body = entry.group(4);
                check(number(body, "min") == spec.min && number(body, "max") == spec.max
                    && number(body, "step") == spec.step, key + " range");
            }
        }
        List<String> javaKeys = new ArrayList<>();
        for (AndroidLocalSettings.Spec spec : AndroidLocalSettings.specs().values()) if (spec.synced) javaKeys.add(spec.key);
        check(rustKeys.equals(javaKeys), "synced keys differ: Rust " + rustKeys + " vs Java " + javaKeys);
    }

    private static int number(String body, String name) {
        Matcher matcher = Pattern.compile(name + ":\\s*(-?\\d+)").matcher(body);
        check(matcher.find(), name + " missing in " + body);
        return Integer.parseInt(matcher.group(1));
    }

    private static void deleteTree(Path root) throws IOException {
        if (!Files.exists(root)) return;
        try (Stream<Path> paths = Files.walk(root)) {
            for (Path path : paths.sorted((a, b) -> b.getNameCount() - a.getNameCount()).toList()) Files.deleteIfExists(path);
        }
    }
}
