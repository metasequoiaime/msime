import app.msime.android.CustomKeyboardSkin;
import app.msime.android.CustomSkinLibrary;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * 自定义皮肤设计参数的 JSON 往返与按 id 合并。要真实的 org.json（check-host 只有 android.jar 里抛 `Stub!` 的桩），所以类名不以 Smoke 结尾：check-host 只编译它，运行用 `java -cp <classes>:<json.jar> CustomSkinDesignExchangeCheck`。
 */
public final class CustomSkinDesignExchangeCheck {
    public static void main(String[] args) throws Exception {
        JSONObject design = new JSONObject().put("background", 0x151022).put("accent", 0xD4BBFF)
            .put("cornerRadius", 12).put("keyShape", "pebble").put("soundPack", "msime-bubble")
            .put("pressAnimation", "glow");
        CustomKeyboardSkin skin = CustomKeyboardSkin.from(design);
        CustomKeyboardSkin back = CustomKeyboardSkin.from(skin.toJson(true));
        check(back.key().equals(skin.key()), "toJson round-trips the design");
        check("msime-bubble".equals(back.soundPack()) && "glow".equals(back.pressAnimation()),
            "feedback fields round-trip");
        check("default".equals(CustomKeyboardSkin.from(new JSONObject()).soundPack()),
            "missing sound pack reads as default");

        Path root = Files.createTempDirectory("msime-skin-exchange-");
        Path preferences = root.resolve("preferences");
        Path hardlinkPreferences = root.resolve("hardlink-preferences");
        Path customSkins = hardlinkPreferences.resolve("CustomSkins");
        Files.createDirectories(customSkins);
        Path outside = Files.createTempFile("msime-skin-hardlink-", ".json");
        byte[] protectedBytes = "outside-sentinel".getBytes(java.nio.charset.StandardCharsets.UTF_8);
        Files.write(outside, protectedBytes);
        Files.createLink(customSkins.resolve("library.json.pending"), outside);
        check(CustomSkinLibrary.add(hardlinkPreferences, "hardlink", "硬链接", design, 50),
            "hard-linked pending path does not block a fresh temporary write");
        check(java.util.Arrays.equals(protectedBytes, Files.readAllBytes(outside)),
            "hard-linked pending path does not modify the linked file");
        check(CustomSkinLibrary.add(preferences, "a", "晨雾", design, 100), "add a");
        check(CustomSkinLibrary.add(preferences, "b", "夜航", design.put("accent", 0x00FF00), 200),
            "add b");
        String exported = CustomSkinLibrary.exportDesigns(preferences);
        JSONArray values = new JSONArray(exported);
        check(values.length() == 2 && !values.getJSONObject(0).getJSONObject("design").has("photo"),
            "export carries both designs without photos");
        check(values.getJSONObject(1).getLong("updated_at") == 200, "export carries updated_at");
        String limited = CustomSkinLibrary.exportDesigns(preferences,
            values.getJSONObject(0).toString().getBytes(java.nio.charset.StandardCharsets.UTF_8).length + 2);
        check(new JSONArray(limited).length() == 1, "the byte-limited export stops before the limit");
        check("[]".equals(CustomSkinLibrary.exportDesigns(preferences, 1)), "nothing fits in one byte");

        JSONArray incoming = new JSONArray()
            .put(new JSONObject().put("id", "a").put("name", "晨雾·新").put("updated_at", 300)
                .put("design", new JSONObject().put("accent", 0x123456)))
            .put(new JSONObject().put("id", "b").put("name", "夜航·旧").put("updated_at", 150)
                .put("design", new JSONObject()))
            .put(new JSONObject().put("id", "c").put("name", "新皮肤").put("updated_at", 10)
                .put("design", new JSONObject()));
        check(CustomSkinLibrary.importDesigns(preferences, incoming.toString()) == 2,
            "newer a and new c are merged, older b is kept");
        List<CustomSkinLibrary.Item> items = CustomSkinLibrary.read(preferences);
        check(items.size() == 3 && "晨雾·新".equals(items.get(0).name())
            && items.get(0).updatedAt() == 300, "a replaced");
        check("夜航".equals(items.get(1).name()) && items.get(1).updatedAt() == 200, "b kept");
        check("c".equals(items.get(2).id()), "c appended");
        check(CustomSkinLibrary.importDesigns(preferences, "not json") == 0, "junk is ignored");
        System.out.println("Android custom skin exchange: JSON round trip and id merge passed");
    }

    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
}
