package app.msime.android;

import java.io.File;
import java.io.IOException;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import org.json.JSONException;
import org.json.JSONObject;

/** 在偏好工作线程读取并解码候选包，输入线程只取与当前请求相符的缓存。 */
public final class CandidateSkinResolver {
    private record Resolved(String request, String id, Map<String, String> colours,
            CandidateCanvasAssets canvas, CandidateAppearance.Layout layout) {}
    private record Modes(String directory, Resolved light, Resolved dark) {}
    private volatile Modes modes;
    private Map<String, CandidateCanvasAssets.Image> images = Map.of();
    // 下面几项只在偏好工作线程上读写：上次整套读取的目录、读之前取的偏好文件戳与候选包戳、指名的包和得到的响应。
    private String lastDirectory;
    private Object lastPreferencesStamp;
    private List<String> lastPackageIds = List.of();
    private Object lastPackageStamp;
    private String lastResponse;

    /** 与偏好重读共用工作线程；每次重新校验包，资源更新不要求同时修改偏好或清单。 */
    public String read(String directory) {
        return read(directory, true);
    }

    /**
     * {@link #read(String)}；`fresh` 为假时是键盘显示期间每秒一次的轮询：偏好文件和指名候选包里的文件（inode、修改时间、大小）都与上次整套读取时相同，就原样交回上次的响应，不再经 JNI 读偏好、解析皮肤、逐张取图做 base64 和摘要。
     *
     * <p>原先这一套每秒都做，自定义候选皮肤明暗两份各重读、重校验一遍全部图片，低端机上是持续的发热与耗电。设置页保存偏好是原子替换，安装或改包会改到包目录里的条目，所以都会让戳变；键盘每次显示的第一次读取（`fresh`）仍整套重读，戳漏看的同秒同大小改写最多留到下次显示。
     */
    public String read(String directory, boolean fresh) {
        Object preferencesStamp = CandidateSkinStamp.file(Paths.get(directory, "preferences.json"));
        if (!fresh && lastResponse != null && preferencesStamp != null && directory.equals(lastDirectory)
                && preferencesStamp.equals(lastPreferencesStamp) && lastPackageStamp != null
                && lastPackageStamp.equals(packageStamp(directory, lastPackageIds)))
            return lastResponse;
        lastResponse = null;
        String response = NativeClient.loadPreferences(directory);
        Modes next = null;
        Map<String, CandidateCanvasAssets.Image> decoded = new HashMap<>();
        List<String> packageIds = List.of();
        Object packageStamp = null;
        boolean ok = false;
        try {
            JSONObject envelope = new JSONObject(response);
            JSONObject value = JsonPolicy.strictTrue(envelope.opt("ok"))
                ? envelope.optJSONObject("value") : null;
            JSONObject preferences = value == null ? null : value.optJSONObject("preferences");
            if (preferences != null && new File(directory).isAbsolute()
                    && "custom".equals(preferences.optString("global_theme"))) {
                packageIds = packageIds(preferences);
                // 先取戳再读包：读的过程中包被改了，下一次轮询的戳就对不上，会整套再读。
                packageStamp = packageStamp(directory, packageIds);
                String root = new File(directory, "skins").getAbsolutePath();
                next = new Modes(directory, resolve(preferences, root, false, decoded),
                    resolve(preferences, root, true, decoded));
            } else {
                packageStamp = List.of();
            }
            ok = preferences != null;
        } catch (JSONException | RuntimeException | LinkageError ignored) {
            // 候选包失效只恢复现有配色，不阻止偏好和引擎会话更新。
        }
        modes = next;
        images = Map.copyOf(decoded);
        lastDirectory = directory;
        lastPreferencesStamp = preferencesStamp;
        lastPackageIds = packageIds;
        lastPackageStamp = packageStamp;
        // 读或解析失败时不缓存：失败可能是暂时的，下次轮询要真去读。
        lastResponse = ok ? response : null;
        return response;
    }

    /** 浅、深两个槽指名的候选包（`custom_theme.candidate_skin` 与 `candidate_skin_dark`），与共享解析器按明暗取槽的字段相同。 */
    private static List<String> packageIds(JSONObject preferences) {
        JSONObject custom = preferences.optJSONObject("custom_theme");
        if (custom == null) return List.of();
        List<String> ids = new ArrayList<>(2);
        for (String slot : new String[] {"candidate_skin", "candidate_skin_dark"}) {
            String id = JsonPolicy.strictString(custom.opt(slot));
            if (id != null && !id.isEmpty() && !ids.contains(id)) ids.add(id);
        }
        return List.copyOf(ids);
    }

    private static Object packageStamp(String directory, List<String> ids) {
        if (ids.isEmpty()) return List.of();
        return CandidateSkinStamp.packages(Paths.get(directory, "skins"), ids);
    }

    /** 当前解析结果本身；轮询沿用上次结果时它不变，输入线程据此判断候选包有没有重新解析过。 */
    public Object state() {
        return modes;
    }

    private static String request(JSONObject preferences, boolean dark) throws JSONException {
        return KeyboardSkin.themeRequest(preferences.optString("global_theme", "system"),
            preferences.optJSONObject("custom_theme"), dark);
    }

    private Resolved resolve(JSONObject preferences, String root, boolean dark,
            Map<String, CandidateCanvasAssets.Image> decoded)
            throws JSONException {
        String key = request(preferences, dark);
        JSONObject query = new JSONObject(key).put("skins_directory", root);
        JSONObject envelope = new JSONObject(NativeClient.resolveTheme(query.toString()));
        JSONObject value = JsonPolicy.strictTrue(envelope.opt("ok"))
            ? envelope.optJSONObject("value") : null;
        if (value == null) return null;
        String id = JsonPolicy.strictString(value.opt("candidate_skin"));
        if (id == null || id.isEmpty()) return null;
        CandidateCanvasAssets canvas = null;
        try { canvas = CandidateCanvasAssets.load(root, id, dark, decoded, images); }
        catch (IOException | RuntimeException ignored) {
            // 资源失败时沿用解析器的不透明配色，不把贴图专用文字画到键盘底色上。
        }
        if (canvas != null && canvas.panelDrawn()) {
            JSONObject panelResponse = new JSONObject(NativeClient.resolveTheme(
                query.put("canvas_panel_drawn", true).toString()));
            JSONObject panelValue = JsonPolicy.strictTrue(panelResponse.opt("ok"))
                ? panelResponse.optJSONObject("value") : null;
            if (panelValue != null && id.equals(panelValue.optString("candidate_skin"))) value = panelValue;
            else canvas = null;
        }
        JSONObject palette = value.optJSONObject("candidate");
        // 仅含图片的 system 基底包没有显式候选配色，仍保留已确认生效的包与宿主安全配色。
        Map<String, String> colours = new HashMap<>();
        for (String name : new String[] {"surface", "border", "text", "number", "secondary",
                "accent", "selected", "selected_text", "selected_number", "hover"}) {
            String colour = palette == null ? null : JsonPolicy.strictString(palette.opt(name));
            if (colour != null) colours.put(name, colour);
        }
        JSONObject details = value.optJSONObject("candidate_details");
        if (details != null) for (String name : new String[] {"preedit_text", "preedit_caret", "selected_translation"}) {
            String colour = JsonPolicy.strictString(details.opt(name));
            if (colour != null) colours.put(name, colour);
        }
        CandidateAppearance.Layout layout = CandidateAppearance.Layout.from(value.optJSONObject("candidate_layout"),
            CandidateAppearance.fontSize(NumberPolicy.strictInt(preferences, "candidate_font_size", 16)));
        return new Resolved(key, id, Map.copyOf(colours), canvas, layout);
    }

    /** 不触碰文件系统，也不把旧请求的包颜色套到刚切换的主题上。 */
    public CandidateAppearance.Palette paletteFor(String directory, JSONObject preferences,
            boolean dark, CandidateAppearance.Palette fallback) {
        Modes current = modes;
        if (current == null || preferences == null || !current.directory().equals(directory))
            return fallback;
        Resolved selected = dark ? current.dark() : current.light();
        if (selected == null) return fallback;
        try {
            if (selected.request().equals(request(preferences, dark)))
                return CandidateAppearance.fromResolved(fallback, selected.id(), selected.colours(), selected.canvas(), selected.layout());
        } catch (JSONException ignored) {
            // 与键盘相同，非法主题请求不保留另一份主题的颜色。
        }
        return fallback;
    }
}
