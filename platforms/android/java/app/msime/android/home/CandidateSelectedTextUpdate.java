package app.msime.android.home;

import app.msime.android.CandidateSelectedTextPolicy;
import app.msime.android.JsonPolicy;
import app.msime.android.NativeClient;
import org.json.JSONException;
import org.json.JSONObject;

/** 把共享层的选色补丁适配到本次刚读出的偏好，保存和 CAS 仍走 KeyboardSheets。 */
final class CandidateSelectedTextUpdate {
    private CandidateSelectedTextUpdate() {}

    static void apply(JSONObject preferences, String value) throws JSONException {
        JSONObject request = new JSONObject().put("current", preferences.opt("global_theme"))
            .put("value", value == null ? JSONObject.NULL : value);
        JSONObject patch;
        try {
            patch = JsonPolicy.objectValue(new JSONObject(NativeClient.candidateColorUpdate(request.toString())));
        } catch (IllegalArgumentException | LinkageError error) {
            throw invalid();
        }
        if (patch == null || !patch.has("selected_text") || !(patch.opt("reset_skins") instanceof Boolean))
            throw invalid();
        String selected = patch.isNull("selected_text") ? null : JsonPolicy.strictString(patch.opt("selected_text"));
        if (!patch.isNull("selected_text") && CandidateSelectedTextPolicy.normalized(selected) == null)
            throw invalid();
        String global = optionalString(patch, "global_theme");
        String base = optionalString(patch, "base");
        boolean resetSkins = (Boolean) patch.opt("reset_skins");

        JSONObject custom = KeyboardSheets.child(preferences, "custom_theme");
        JSONObject colors = KeyboardSheets.child(custom, "candidate_colors");
        if (selected == null) colors.remove(CandidateSelectedTextPolicy.SLOT);
        else colors.put(CandidateSelectedTextPolicy.SLOT, selected);
        if (global != null) preferences.put("global_theme", global);
        if (base != null) custom.put("base", base);
        if (resetSkins) {
            custom.remove("candidate_skin");
            custom.remove("candidate_skin_dark");
        }
    }

    private static String optionalString(JSONObject patch, String key) throws JSONException {
        if (!patch.has(key)) return null;
        String value = JsonPolicy.strictString(patch.opt(key));
        if (value == null) throw invalid();
        return value;
    }

    private static JSONException invalid() { return new JSONException("candidate_color_update_failed"); }
}
