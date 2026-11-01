package app.msime.android.home;

import android.widget.LinearLayout;
import androidx.annotation.Nullable;
import java.util.LinkedHashSet;
import java.util.Set;
import org.json.JSONArray;
import org.json.JSONObject;

/** 按共享偏好的规则 ID 逐项配置模糊音；总开关关闭时仍保留勾选。 */
public final class FuzzyPinyinPage extends ReloadingDetailPage {
    private static final String[][][] GROUPS = {
        {{"平翘舌"}, {"z-zh", "z ↔ zh"}, {"c-ch", "c ↔ ch"}, {"s-sh", "s ↔ sh"}},
        {{"声母"}, {"n-l", "n ↔ l"}, {"f-h", "f ↔ h"}, {"r-l", "r ↔ l"}},
        {{"前后鼻音"}, {"an-ang", "an ↔ ang"}, {"en-eng", "en ↔ eng"}, {"in-ing", "in ↔ ing"}},
        {{"其他韵母"}, {"ian-iang", "ian ↔ iang"}, {"uan-uang", "uan ↔ uang"}},
    };

    @Override protected void reload() {
        HostTask.run(this, KeyboardSheets::preferences, this::render);
    }

    private void render(@Nullable JSONObject preferences) {
        LinearLayout target = contentColumn();
        if (target == null) return;
        target.removeAllViews();
        if (preferences == null) {
            GroupCard.add(target, null).note("读取设置失败，请稍后重试");
            return;
        }
        JSONObject fuzzy = preferences.optJSONObject("fuzzy_pinyin");
        boolean enabled = fuzzy != null && fuzzy.optBoolean("enabled", false);
        Set<String> selected = selectedRules(fuzzy);

        GroupCard mode = GroupCard.add(target, "模糊音");
        mode.toggle("启用模糊音", null, enabled,
            checked -> savePreferences(values -> KeyboardSheets.child(values, "fuzzy_pinyin").put("enabled", checked))).help("只按勾选的规则匹配；关闭后保留勾选");
        mode.help("首次开启会勾选全部规则；也可以先在下面只勾选需要的规则，再开启模糊音。");

        for (String[][] entries : GROUPS) {
            GroupCard group = GroupCard.add(target, entries[0][0]);
            for (int index = 1; index < entries.length; index++) {
                String id = entries[index][0];
                String label = entries[index][1];
                group.toggle(label, null, selected.contains(id), checked -> savePreferences(values -> {
                    JSONObject current = KeyboardSheets.child(values, "fuzzy_pinyin");
                    Set<String> rules = selectedRules(current);
                    if (checked) rules.add(id); else rules.remove(id);
                    JSONArray saved = new JSONArray();
                    for (String rule : rules) saved.put(rule);
                    current.put("rules", saved);
                    current.put("seeded", true);
                }));
            }
        }
    }

    private static Set<String> selectedRules(@Nullable JSONObject fuzzy) {
        Set<String> selected = new LinkedHashSet<>();
        JSONArray rules = fuzzy == null ? null : fuzzy.optJSONArray("rules");
        if (rules != null) for (int index = 0; index < rules.length(); index++) {
            String id = rules.optString(index, "");
            if (!id.isEmpty()) selected.add(id);
        }
        return selected;
    }
}
