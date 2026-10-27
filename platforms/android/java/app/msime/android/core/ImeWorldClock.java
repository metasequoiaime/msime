package app.msime.android;

import android.graphics.Color;
import android.icu.text.TimeZoneNames;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.PopupMenu;
import android.widget.TextView;
import java.util.ArrayDeque;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.TimeZone;
import java.util.function.Consumer;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/** 离线时钟的原生面板；规则和落盘由共享 Rust 持有，宿主只管理视图和回调代次。 */
final class ImeWorldClock {
    private final MSIMEInputService s;
    private boolean active;
    private long generation;
    private boolean busy;
    private final ArrayDeque<Runnable> pendingActions = new ArrayDeque<>();
    private JSONArray catalog = new JSONArray();
    private JSONArray selected = new JSONArray();
    private JSONArray cards = new JSONArray();
    private String page = "clock";
    private String formatCity = "system";
    private long pickerGeneration;
    private boolean pickerFinished;
    private String pickerPackage;
    private int pickerField;
    private final Map<String, TextView> cardViews = new LinkedHashMap<>();
    private final Map<String, TextView> formatViews = new LinkedHashMap<>();
    private final Map<String, String> cityLabels = new LinkedHashMap<>();
    private final Runnable tick = this::refresh;

    ImeWorldClock(MSIMEInputService s) { this.s = s; }
    boolean active() { return active; }

    void open() {
        s.closeToolbarPanels();
        active = true;
        page = "clock";
        selected = new JSONArray();
        cards = new JSONArray();
        build();
        ViewPolicy.show(s.moreToolsScroll);
        s.render();
        request(action("catalog"), value -> {
            catalog = (JSONArray) value;
            localizeCities();
            request(action("load"), loaded -> {
                selected = ((JSONObject) loaded).optJSONArray("cities");
                if (selected == null) selected = new JSONArray();
                refresh();
            });
        });
    }

    void close() {
        active = false;
        generation++;
        busy = false;
        pendingActions.clear();
        s.main.removeCallbacks(tick);
        cardViews.clear();
        formatViews.clear();
    }

    void destroy() {
        close();
        pickerGeneration++;
        pickerPackage = null;
    }

    /** 选择页有自己的输入连接；结束后只在原编辑器恢复时重开时钟。 */
    void resumePicker(android.view.inputmethod.EditorInfo info) {
        if (!pickerFinished || info == null || WorldClockCityActivity.SEARCH_EDITOR.equals(info.privateImeOptions)
                || !s.isInputViewShown() || s.preferencesDirectory == null || s.preferencesDirectory.isEmpty()) return;
        if (!java.util.Objects.equals(pickerPackage, info.packageName) || pickerField != info.fieldId) return;
        pickerFinished = false;
        pickerPackage = null;
        open();
    }

    /** 常规 render 不重建时钟，保留滚动位置和正在触摸的视图。 */
    void render() {
        if (active && s.moreToolsPanel.getChildCount() == 0) build();
    }

    private static JSONObject action(String operation) {
        JSONObject action = new JSONObject();
        try { action.put("operation", operation); }
        catch (JSONException impossible) { throw new IllegalStateException(impossible); }
        return action;
    }

    private void request(JSONObject action, Consumer<Object> success) {
        if (!active || busy) return;
        final String directory = s.preferencesDirectory;
        if (directory == null || directory.isEmpty()) { error("时钟尚未就绪"); return; }
        final long expected = generation;
        final Object connection = s.connection;
        busy = true;
        try {
            JSONObject request = new JSONObject().put("directory", directory).put("action", action);
            s.preferencesWorker.execute(() -> {
                Object value = null;
                String failure = "";
                try {
                    JSONObject response = new JSONObject(NativeClient.worldClock(request.toString()));
                    if (response.optBoolean("ok")) value = response.get("value");
                    else failure = response.optString("error", "world_clock_io");
                } catch (JSONException | RuntimeException | LinkageError unavailable) { failure = "world_clock_io"; }
                final Object result = value;
                final String reason = failure;
                s.main.post(() -> {
                    if (!active || expected != generation || connection != s.connection) return;
                    busy = false;
                    if (result == null) {
                        pendingActions.clear();
                        error(switch (reason) {
                            case "world_clock_limit" -> "最多添加 32 个城市";
                            case "world_clock_corrupt" -> "城市记录损坏，原文件已保留";
                            default -> "无法读取或保存时钟，请重试";
                        });
                        return;
                    }
                    success.accept(result);
                    drainActions();
                });
            });
        } catch (JSONException | java.util.concurrent.RejectedExecutionException unavailable) {
            busy = false;
            error("时钟暂时不可用");
        }
    }

    /** 定时快照占用工作线程时保留用户操作；关闭面板时一并作废。 */
    private void whenIdle(Runnable action) {
        if (!active) return;
        pendingActions.addLast(action);
        drainActions();
    }

    private void drainActions() {
        while (active && !busy && !pendingActions.isEmpty()) pendingActions.removeFirst().run();
    }

    private JSONObject snapshotAction() {
        JSONObject action = action("snapshot");
        long now = System.currentTimeMillis();
        TimeZone zone = TimeZone.getDefault();
        try {
            action.put("timestamp", now / 1000).put("system_zone", zone.getID())
                .put("system_offset", zone.getOffset(now) / 1000).put("cities", selected);
        } catch (JSONException impossible) { throw new IllegalStateException(impossible); }
        return action;
    }

    private void refresh() {
        if (!active || !(page.equals("clock") || page.equals("formats"))) return;
        s.main.removeCallbacks(tick);
        if (busy) { s.main.postDelayed(tick, 1000); return; }
        request(snapshotAction(), value -> {
            cards = (JSONArray) value;
            if (page.equals("clock") && cardViews.size() != cards.length()) build();
            update();
            s.main.postDelayed(tick, 1000 - System.currentTimeMillis() % 1000);
        });
    }

    private void localizeCities() {
        cityLabels.clear();
        TimeZoneNames names = TimeZoneNames.getInstance(Locale.SIMPLIFIED_CHINESE);
        for (int i = 0; i < catalog.length(); i++) {
            JSONObject city = catalog.optJSONObject(i);
            if (city == null) continue;
            String id = city.optString("id");
            String zone = city.optString("zone");
            String label = id.equals("city:beijing") ? "北京" : names.getExemplarLocationName(zone);
            if (label == null || label.isEmpty()) label = zone.substring(zone.lastIndexOf('/') + 1).replace('_', ' ');
            cityLabels.put(id, label);
        }
    }

    private String name(String id) {
        return id.equals("system") ? "系统时间" : cityLabels.getOrDefault(id, id);
    }

    private void build() {
        if (!active) return;
        cardViews.clear();
        formatViews.clear();
        s.moreToolsPanel.removeAllViews();
        s.moreToolsPanel.setContentDescription("世界时钟面板");
        LinearLayout header = LayoutPolicy.row(s);
        s.button(header, "返回", () -> {
            if (page.equals("clock")) {
                close();
                s.imeFunctionPanel.renderMoreTools();
            } else { page = "clock"; build(); refresh(); }
        });
        TextView title = text(page.equals("formats") ? name(formatCity) + " · 插入格式" : "世界时钟", 16);
        header.addView(title, new LinearLayout.LayoutParams(0, s.pixels(44), 1));
        if (page.equals("clock")) s.button(header, "添加城市", () -> whenIdle(() -> {
            s.main.removeCallbacks(tick);
            page = "regions";
            regions();
        }));
        s.moreToolsPanel.addView(header);
        if (page.equals("formats")) {
            JSONObject card = findCard(formatCity);
            if (card == null) return;
            JSONArray formats = card.optJSONArray("formats");
            if (formats == null) return;
            for (int i = 0; i < formats.length(); i++) {
                JSONObject format = formats.optJSONObject(i);
                if (format == null) continue;
                String label = format.optString("label");
                Button button = row(label + "\n" + format.optString("text"), () -> insert(label));
                formatViews.put(label, button);
            }
        } else if (page.equals("clock")) {
            if (cards.length() == 0) s.moreToolsPanel.addView(text("正在加载…", 14));
            for (int i = 0; i < cards.length(); i++) {
                JSONObject card = cards.optJSONObject(i);
                if (card == null) continue;
                String id = card.optString("id");
                Button button = row("", () -> whenIdle(() -> {
                    formatCity = id;
                    page = "formats";
                    build();
                    refresh();
                }));
                button.setContentDescription(name(id) + "时钟");
                if (!id.equals("system")) button.setOnLongClickListener(view -> {
                    s.imeKeyFeedback.playFeedback(view);
                    whenIdle(() -> {
                        PopupMenu menu = new PopupMenu(s, view);
                        menu.getMenu().add("删除" + name(id)).setOnMenuItemClickListener(item -> {
                            mutate("remove", id);
                            return true;
                        });
                        menu.show();
                    });
                    return true;
                });
                cardViews.put(id, button);
            }
            s.moreToolsPanel.addView(text("点时钟选择格式插入 · 长按城市删除\n日出日落为标注城市的近似值，不使用定位", 11));
            update();
        }
        s.moreToolsScroll.scrollTo(0, 0);
    }

    private JSONObject findCard(String id) {
        for (int i = 0; i < cards.length(); i++) {
            JSONObject card = cards.optJSONObject(i);
            if (card != null && card.optString("id").equals(id)) return card;
        }
        return null;
    }

    private void update() {
        if (page.equals("formats")) {
            JSONObject card = findCard(formatCity);
            JSONArray formats = card == null ? null : card.optJSONArray("formats");
            if (formats == null) return;
            for (int i = 0; i < formats.length(); i++) {
                JSONObject format = formats.optJSONObject(i);
                if (format == null) continue;
                String label = format.optString("label");
                TextView view = formatViews.get(label);
                if (view != null) view.setText(label + "\n" + format.optString("text"));
            }
            return;
        }
        if (!page.equals("clock")) return;
        for (int i = 0; i < cards.length(); i++) {
            JSONObject card = cards.optJSONObject(i);
            if (card == null) continue;
            String id = card.optString("id");
            TextView view = cardViews.get(id);
            if (view == null) continue;
            String reference = JsonPolicy.strictStringOrEmpty(card.opt("solar_reference"));
            String sunrise = JsonPolicy.strictStringOrEmpty(card.opt("sunrise"));
            String sunset = JsonPolicy.strictStringOrEmpty(card.opt("sunset"));
            String solar = reference.isEmpty() ? "日出日落：此时区没有参考城市"
                : "日出 " + (sunrise.isEmpty() ? "当日无" : sunrise) + " · 日落 " + (sunset.isEmpty() ? "当日无" : sunset)
                    + "（参考" + name(reference) + "）";
            view.setText(name(id) + "   " + card.optString("time") + "\n" + card.optString("date")
                + " · " + card.optString("offset") + "\n" + solar);
        }
    }

    private void insert(String label) {
        whenIdle(() -> insertNow(label));
    }

    private void insertNow(String label) {
        if (s.connection == null) return;
        s.main.removeCallbacks(tick);
        // 点按时重新取时钟，防止停留在格式页时插入旧值；请求代次也绑定当前输入连接。
        request(snapshotAction(), value -> {
            cards = (JSONArray) value;
            JSONObject card = findCard(formatCity);
            JSONArray formats = card == null ? null : card.optJSONArray("formats");
            if (formats == null) return;
            for (int i = 0; i < formats.length(); i++) {
                JSONObject format = formats.optJSONObject(i);
                if (format != null && format.optString("label").equals(label)) {
                    s.command(2);
                    if (s.commitText(format.optString("text"), TypingSource.LOCAL)) {
                        s.closeMoreTools();
                        s.render();
                    } else refresh();
                    return;
                }
            }
        });
    }

    private void mutate(String operation, String id) {
        whenIdle(() -> mutateNow(operation, id));
    }

    private void mutateNow(String operation, String id) {
        s.main.removeCallbacks(tick);
        JSONObject action = action(operation);
        try { action.put("id", id); }
        catch (JSONException impossible) { throw new IllegalStateException(impossible); }
        request(action, value -> {
            selected = ((JSONObject) value).optJSONArray("cities");
            if (selected == null) selected = new JSONArray();
            page = "clock";
            cards = new JSONArray();
            build();
            refresh();
        });
    }

    private void regions() {
        build();
        for (String region : List.of("Asia", "Europe", "America", "Africa", "Australia", "Pacific", "Atlantic", "Indian", "Antarctica", "Arctic")) {
            row(regionName(region), () -> countries(region));
        }
    }

    private static String regionName(String region) {
        return switch (region) {
            case "Asia" -> "亚洲"; case "Europe" -> "欧洲"; case "America" -> "美洲";
            case "Africa" -> "非洲"; case "Australia" -> "澳大利亚"; case "Pacific" -> "太平洋";
            case "Atlantic" -> "大西洋"; case "Indian" -> "印度洋"; case "Antarctica" -> "南极洲";
            default -> "北极";
        };
    }

    private void countries(String region) {
        android.view.inputmethod.EditorInfo info = s.getCurrentInputEditorInfo();
        if (info == null) return;
        pickerPackage = info.packageName;
        pickerField = info.fieldId;
        pickerFinished = false;
        long expected = ++pickerGeneration;
        android.os.ResultReceiver receiver = new android.os.ResultReceiver(s.main) {
            @Override protected void onReceiveResult(int code, android.os.Bundle value) {
                if (expected != pickerGeneration || pickerPackage == null) return;
                pickerFinished = true;
                resumePicker(s.getCurrentInputEditorInfo());
            }
        };
        android.content.Intent intent = new android.content.Intent(s, WorldClockCityActivity.class)
            .addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK | android.content.Intent.FLAG_ACTIVITY_MULTIPLE_TASK
                | android.content.Intent.FLAG_ACTIVITY_EXCLUDE_FROM_RECENTS)
            .putExtra("region", region).putExtra("catalog", catalog.toString()).putExtra("selected", selected.toString())
            .putExtra("directory", s.preferencesDirectory).putExtra("result", receiver);
        try {
            s.closeMoreTools();
            s.startActivity(intent);
        } catch (RuntimeException unavailable) {
            pickerPackage = null;
            open();
            error("无法打开城市选择页，请重试");
        }
    }

    private TextView text(String value, int size) {
        TextView view = new TextView(s);
        view.setText(value);
        view.setTextSize(size);
        view.setTextColor(Color.parseColor(s.skin.keyForeground()));
        view.setGravity(android.view.Gravity.CENTER_VERTICAL);
        view.setPadding(s.pixels(8), s.pixels(6), s.pixels(8), s.pixels(6));
        return view;
    }

    private Button row(String value, Runnable action) {
        Button button = s.button(s.moreToolsPanel, value, action);
        button.setTextColor(Color.parseColor(s.skin.keyForeground()));
        button.setGravity(android.view.Gravity.START | android.view.Gravity.CENTER_VERTICAL);
        KeyboardTypographyPolicy.setKeyTextSize(button, 14);
        MSIMEInputService.role(button, KeyboardKeyRole.KEY);
        s.imeStyler.styleButton(button, KeyboardKeyRole.KEY, s.skin);
        button.setAllCaps(false);
        button.setSingleLine(false);
        button.setMaxLines(5);
        // 时钟卡片固定有时间、日期和日出日落三行；预留三行高度，避免 Android 11 的 Button 默认最小高度裁掉中文下半部。
        button.setMinHeight(s.pixels(76));
        button.setPadding(s.pixels(10), s.pixels(8), s.pixels(10), s.pixels(8));
        button.setLayoutParams(new LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT));
        return button;
    }

    private void error(String message) {
        s.moreToolsPanel.addView(text(message, 14));
        row("重新加载", this::open);
    }
}
