package app.msime.android;

import android.app.Activity;
import android.os.Bundle;
import android.os.ResultReceiver;
import android.icu.text.TimeZoneNames;
import android.view.inputmethod.InputMethodManager;
import android.widget.Button;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;
import java.text.Collator;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/** 国家搜索需要普通 Activity 的输入连接，不能把文本框放在输入法自己的窗口里。 */
@SuppressWarnings("deprecation")
public final class WorldClockCityActivity extends Activity {
    static final String SEARCH_EDITOR = "app.msime.android.world-clock-search";
    private final ExecutorService worker = Executors.newSingleThreadExecutor();
    private final Collator collator = Collator.getInstance(Locale.SIMPLIFIED_CHINESE);
    private JSONArray catalog;
    private JSONArray selected;
    private String region;
    private String query = "";
    private String country = "";
    private EditText search;
    private LinearLayout results;
    private TextView status;
    private boolean saving;
    private boolean sent;

    @Override protected void onCreate(Bundle state) {
        super.onCreate(state);
        // 「防止截屏」（#10098）：窗口显示前按本地设置加上 FLAG_SECURE。
        ScreenCaptureProtection.apply(this);
        try {
            catalog = new JSONArray(getIntent().getStringExtra("catalog"));
            selected = new JSONArray(getIntent().getStringExtra("selected"));
            region = getIntent().getStringExtra("region");
            if (region == null || getIntent().getStringExtra("directory") == null) { finish(); return; }
        } catch (JSONException | RuntimeException invalid) { finish(); return; }
        if (state != null) {
            query = state.getString("query", "");
            country = state.getString("country", "");
        }
        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        int padding = Math.round(12 * getResources().getDisplayMetrics().density);
        root.setPadding(padding, padding, padding, padding);
        Button back = new Button(this);
        back.setText("返回");
        back.setOnClickListener(view -> onBackPressed());
        root.addView(back);
        TextView title = new TextView(this);
        title.setText("世界时钟 · 选择国家或地区");
        title.setTextSize(18);
        root.addView(title);
        LinearLayout row = new LinearLayout(this);
        search = new EditText(this);
        search.setSingleLine(true);
        search.setHint("输入国家名称");
        search.setContentDescription("按国家名称搜索");
        search.setPrivateImeOptions(SEARCH_EDITOR);
        search.setText(query);
        row.addView(search, new LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1));
        Button submit = new Button(this);
        submit.setText("搜索");
        submit.setOnClickListener(view -> {
            if (saving) return;
            query = search.getText().toString().trim();
            country = "";
            showCountries();
        });
        row.addView(submit);
        root.addView(row);
        status = new TextView(this);
        root.addView(status);
        ScrollView scroll = new ScrollView(this);
        results = new LinearLayout(this);
        results.setOrientation(LinearLayout.VERTICAL);
        scroll.addView(results);
        root.addView(scroll, new LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, 0, 1));
        setContentView(root);
        if (country.isEmpty()) showCountries(); else showCities();
    }

    private void showCountries() {
        results.removeAllViews();
        status.setText("选择国家或地区");
        Map<String, String> countries = new LinkedHashMap<>();
        for (int i = 0; i < catalog.length(); i++) {
            JSONObject city = catalog.optJSONObject(i);
            if (city == null || !city.optString("zone").startsWith(region + "/")) continue;
            String code = city.optString("country");
            String label = new Locale("", code).getDisplayCountry(Locale.SIMPLIFIED_CHINESE);
            if (query.isEmpty() || label.contains(query)) countries.put(code, label);
        }
        List<Map.Entry<String, String>> entries = new ArrayList<>(countries.entrySet());
        entries.sort((a, b) -> collator.compare(a.getValue(), b.getValue()));
        if (entries.isEmpty()) status.setText("没有匹配的国家或地区");
        for (Map.Entry<String, String> entry : entries) button(entry.getValue(), () -> {
            country = entry.getKey();
            getSystemService(InputMethodManager.class).hideSoftInputFromWindow(search.getWindowToken(), 0);
            search.clearFocus();
            showCities();
        }, true);
    }

    private void showCities() {
        results.removeAllViews();
        status.setText("选择城市（离线）");
        List<JSONObject> cities = new ArrayList<>();
        for (int i = 0; i < catalog.length(); i++) {
            JSONObject city = catalog.optJSONObject(i);
            if (city != null && city.optString("country").equals(country)
                    && city.optString("zone").startsWith(region + "/")) cities.add(city);
        }
        TimeZoneNames names = TimeZoneNames.getInstance(Locale.SIMPLIFIED_CHINESE);
        cities.sort((a, b) -> collator.compare(label(names, a), label(names, b)));
        for (JSONObject city : cities) {
            String id = city.optString("id");
            boolean added = false;
            for (int i = 0; i < selected.length(); i++) if (id.equals(selected.optString(i))) added = true;
            button(label(names, city) + " · " + city.optString("zone") + (added ? "（已添加）" : ""),
                () -> save(id), !added);
        }
    }

    private static String label(TimeZoneNames names, JSONObject city) {
        String zone = city.optString("zone");
        String value = city.optString("id").equals("city:beijing") ? "北京" : names.getExemplarLocationName(zone);
        return value == null || value.isEmpty() ? zone.substring(zone.lastIndexOf('/') + 1).replace('_', ' ') : value;
    }

    private void button(String text, Runnable action, boolean enabled) {
        Button button = new Button(this);
        button.setText(text);
        button.setAllCaps(false);
        button.setEnabled(enabled);
        button.setOnClickListener(view -> { if (!saving) action.run(); });
        results.addView(button, new LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT));
    }

    private void save(String id) {
        if (saving) return;
        saving = true;
        status.setText("正在添加城市…");
        String directory = getIntent().getStringExtra("directory");
        worker.execute(() -> {
            String error = "world_clock_io";
            try {
                JSONObject action = new JSONObject().put("operation", "add").put("id", id);
                JSONObject request = new JSONObject().put("directory", directory).put("action", action);
                JSONObject response = new JSONObject(NativeClient.worldClock(request.toString()));
                error = response.optBoolean("ok") ? "" : response.optString("error", "world_clock_io");
            } catch (JSONException | RuntimeException | LinkageError unavailable) { }
            String failure = error;
            runOnUiThread(() -> {
                if (isDestroyed()) return;
                saving = false;
                if (failure.isEmpty()) finish();
                else status.setText(failure.equals("world_clock_limit") ? "最多添加 32 个城市"
                    : failure.equals("world_clock_corrupt") ? "城市记录损坏，原文件已保留" : "无法保存时钟，请重试");
            });
        });
    }

    @Override protected void onSaveInstanceState(Bundle state) {
        state.putString("query", search.getText().toString());
        state.putString("country", country);
        super.onSaveInstanceState(state);
    }

    @Override public void onBackPressed() {
        if (!country.isEmpty() && !saving) { country = ""; showCountries(); }
        else if (!saving) finish();
    }

    @Override public void finish() {
        if (!sent) {
            sent = true;
            ResultReceiver receiver = getIntent().getParcelableExtra("result");
            if (receiver != null) receiver.send(RESULT_OK, null);
        }
        super.finish();
    }

    @Override protected void onDestroy() {
        worker.shutdown();
        super.onDestroy();
    }
}
