package app.msime.android;

import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/** 只转换共享偏好中的动作身份；默认排列与保存校验由 Rust 提供。 */
public final class TextEditButtonOrder {
    private TextEditButtonOrder() {}

    public static List<TextEditPanelModel.Action> defaults() {
        try {
            JSONObject response = new JSONObject(NativeClient.defaultPreferences());
            if (!response.getBoolean("ok")) throw new JSONException("Defaults unavailable");
            return read(response.getJSONObject("value").getJSONArray(TextEditPanelModel.ORDER_PREFERENCE));
        } catch (JSONException error) {
            throw new IllegalStateException("Text edit defaults unavailable", error);
        }
    }

    public static List<TextEditPanelModel.Action> fromPreferences(JSONObject preferences) {
        JSONArray values = preferences == null ? null : preferences.optJSONArray(TextEditPanelModel.ORDER_PREFERENCE);
        if (values == null) return defaults();
        try { return read(values); }
        catch (JSONException | IllegalArgumentException error) { return defaults(); }
    }

    private static List<TextEditPanelModel.Action> read(JSONArray values) throws JSONException {
        List<TextEditPanelModel.Action> result = new ArrayList<>();
        for (int index = 0; index < values.length(); index++)
            result.add(TextEditPanelModel.Action.valueOf(values.getString(index).toUpperCase(Locale.ROOT)));
        return List.copyOf(result);
    }

    public static JSONArray encode(List<TextEditPanelModel.Action> order) {
        JSONArray result = new JSONArray();
        for (TextEditPanelModel.Action action : order) result.put(action.name().toLowerCase(Locale.ROOT));
        return result;
    }
}
