package app.msime.android.home;

import android.content.Context;
import android.graphics.Color;
import android.graphics.drawable.GradientDrawable;
import android.os.Bundle;
import android.view.View;
import android.view.ViewGroup;
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.annotation.Nullable;
import androidx.core.view.ViewCompat;
import app.msime.android.CustomKeyboardSkin;
import app.msime.android.CustomSkinLibrary;
import app.msime.android.KeyboardSkin;
import app.msime.android.ViewPolicy;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.List;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 皮肤页：两列皮肤卡（预览加名字，选中的卡 2dp 强调色描边、名字前打 ✓），依次是共享目录里的全局主题、「我的设计」（自定义皮肤库，含社区下载和 AI 设计的皮肤），最后一张虚线卡进 AI 设计皮肤。
 *
 * <p>点一张卡立刻生效：全局主题写 `global_theme`；我的设计按键盘自己的皮肤面板那样写进 `custom_theme.keyboard` 并选中 `custom`，同时按 P23 写设计带的按键动画和音效。每次换皮肤都在统计里记一次 `record_skin`，并标记云同步的「皮肤」分类。水杉四季那张卡跟着应用主题当前的季节写成「水杉四季 · 秋杉」。卡片预览用与键盘相同的解析器（{@link HostStore#keyboardSkin(JSONObject, boolean, app.msime.android.AppThemePalette.Seed)}），跟随系统那张按应用主题的种子色画。
 */
public final class SkinsPage extends DetailPage {
    /** 一张卡：`design` 为 null 是全局主题，否则是皮肤库里的设计。 */
    private record Card(String id, String title, KeyboardSkin skin, @Nullable JSONObject design, boolean selected) {}

    private record Model(List<Card> themes, List<Card> designs) {}

    @Nullable private LinearLayout column;

    @Override protected void buildContent(LinearLayout column, Bundle args) {
        this.column = column;
        reload();
    }

    @Override protected void onBecameVisible() {
        if (column != null) reload();
    }

    @Override public void onDestroyView() {
        column = null;
        super.onDestroyView();
    }

    private void reload() {
        boolean dark = AppMode.dark(requireContext());
        String season = KeyboardSheets.seasonTitle(AppThemeController.cachedSeason(requireContext()));
        HostTask.run(this, context -> read(context, dark, season), this::render);
    }

    @Nullable private static Model read(Context context, boolean systemDark, String season) {
        JSONObject preferences = KeyboardSheets.preferences(context);
        if (preferences == null) return null;
        String current = preferences.optString("global_theme", "system");
        JSONArray catalog = HostStore.themeCatalog();
        List<Card> themes = new ArrayList<>(catalog.length());
        try {
            for (int index = 0; index < catalog.length(); index++) {
                JSONObject entry = catalog.optJSONObject(index);
                if (entry == null) continue;
                String id = entry.optString("id", "");
                // 「自定义」由下面的我的设计代替：它画的就是当前选中的那个设计。
                if (id.isEmpty() || "custom".equals(id)) continue;
                String title = entry.optString("title", id);
                if (entry.optBoolean("seasonal", false)) title = title + " · " + season;
                JSONObject probe = new JSONObject(preferences.toString()).put("global_theme", id);
                KeyboardSkin skin = HostStore.keyboardSkin(probe, systemDark, HostStore.seed(context));
                themes.add(new Card(id, title, skin, null, id.equals(current)));
            }
        } catch (JSONException error) {
            return null;
        }

        List<Card> designs = List.of();
        JSONObject customTheme = preferences.optJSONObject("custom_theme");
        JSONObject stored = customTheme == null ? null : customTheme.optJSONObject("keyboard");
        String storedKey = "custom".equals(current) && stored != null ? CustomKeyboardSkin.from(stored).key() : "";
        String directory = HostStore.directory(context);
        if (!directory.isEmpty()) {
            try {
                List<CustomSkinLibrary.Item> storedDesigns = CustomSkinLibrary.read(Paths.get(directory));
                designs = new ArrayList<>(storedDesigns.size());
                for (CustomSkinLibrary.Item item : storedDesigns) {
                    boolean selected = !storedKey.isEmpty()
                        && storedKey.equals(CustomKeyboardSkin.from(item.design()).key());
                    designs.add(new Card(item.id(), item.name(), KeyboardSkin.custom(item.design(), systemDark),
                        item.design(), selected));
                }
            } catch (java.io.IOException error) {
                // 皮肤库读不出来时只少了我的设计这一组，内置主题照常显示。
                android.util.Log.w("MSIMESettings", "Custom skin library unreadable", error);
            }
        }
        return new Model(themes, designs);
    }

    private void render(@Nullable Model model) {
        LinearLayout target = column;
        if (target == null) return;
        target.removeAllViews();
        if (model == null) {
            GroupCard.add(target, null).note("读取设置失败。请先完成首次设置，或稍后返回重试。");
            return;
        }
        Context context = requireContext();
        List<View> themeCards = new ArrayList<>(model.themes().size() + 1);
        for (Card card : model.themes()) themeCards.add(card(context, card));
        if (!AiSkinPage.hidden(context)) themeCards.add(aiCard(context));
        grid(target, "皮肤", themeCards);
        if (!model.designs().isEmpty()) {
            List<View> designCards = new ArrayList<>(model.designs().size());
            for (Card card : model.designs()) designCards.add(card(context, card));
            grid(target, "我的设计", designCards);
        }
    }

    /** 一组两列卡片；组标题与其他详情页的组标题一样，卡片直接放在页面底色上。 */
    private static void grid(LinearLayout target, String title, List<View> cards) {
        GroupCard group = GroupCard.add(target, title);
        LinearLayout holder = group.card();
        ViewPolicy.clearBackground(holder);
        holder.setClipToOutline(false);
        Context context = target.getContext();
        for (int start = 0; start < cards.size(); start += 2) {
            LinearLayout row = Ui.row(context);
            row.setBaselineAligned(false);
            for (int slot = 0; slot < 2; slot++) {
                int index = start + slot;
                View cell = index < cards.size() ? cards.get(index) : new View(context);
                LinearLayout.LayoutParams params = Ui.weightWrap(1f);
                if (slot == 1) params.setMarginStart(Ui.dp(context, 12));
                row.addView(cell, params);
            }
            LinearLayout.LayoutParams rowParams = Ui.matchWidth();
            if (start > 0) rowParams.topMargin = Ui.dp(context, 14);
            holder.addView(row, rowParams);
        }
    }

    private View card(Context context, Card card) {
        LinearLayout cell = Ui.column(context);
        ViewPolicy.setCenteredHorizontally(cell);

        FrameLayout tile = new FrameLayout(context);
        int ring = Ui.dp(context, 2);
        GradientDrawable frame = Ui.outlined(Color.TRANSPARENT, Ui.dp(context, 14),
            card.selected() ? ring : Ui.atLeastOnePx(context, 1),
            card.selected() ? Ui.accent(context) : Ui.outline(context));
        tile.setBackground(frame);
        tile.setPadding(ring + Ui.dp(context, 1), ring + Ui.dp(context, 1), ring + Ui.dp(context, 1),
            ring + Ui.dp(context, 1));
        SkinSwatchView swatch = new SkinSwatchView(context);
        swatch.setSkin(card.skin());
        Ui.hideFromAccessibility(swatch);
        tile.addView(swatch, Ui.frameMatchWidthHeight(context, 76));
        cell.addView(tile, Ui.matchWidth());

        TextView name = Ui.styledLabel(context, card.selected() ? "✓ " + card.title() : card.title(),
            Ui.TEXT_ROW_SUBTITLE + 1, card.selected() ? 600 : 400,
            card.selected() ? Ui.accent(context) : Ui.text(context));
        ViewPolicy.setCentered(name);
        name.setSingleLine(true);
        name.setEllipsize(android.text.TextUtils.TruncateAt.END);
        LinearLayout.LayoutParams nameParams = Ui.matchWidth();
        nameParams.topMargin = Ui.dp(context, 8);
        cell.addView(name, nameParams);

        cell.setContentDescription("皮肤 " + card.title());
        ViewCompat.setStateDescription(cell, card.selected() ? "已选中" : "未选中");
        Ui.makeClickable(cell, context, () -> {
            if (!card.selected()) select(card);
        });
        return cell;
    }

    /** 虚线卡：「✦ 描述一句话生成」，名字是「AI 设计皮肤」，点了进 AI 设计页。 */
    private View aiCard(Context context) {
        LinearLayout cell = Ui.column(context);
        ViewPolicy.setCenteredHorizontally(cell);
        LinearLayout tile = Ui.column(context);
        ViewPolicy.setCentered(tile);
        GradientDrawable dashed = Ui.outlinedDashed(Ui.accentSoft(context), Ui.dp(context, 14),
            Ui.atLeastOnePx(context, 1.5f), Ui.accent(context), Ui.dp(context, 6),
            Ui.dp(context, 4));
        tile.setBackground(dashed);
        TextView spark = Ui.styledLabel(context, "✦", 22, 400, Ui.accent(context));
        ViewPolicy.setCentered(spark);
        tile.addView(spark);
        TextView hint = Ui.styledLabel(context, "描述一句话生成", 12, 400, Ui.accent(context));
        ViewPolicy.setCentered(hint);
        tile.addView(hint);
        cell.addView(tile, Ui.matchWidthHeightPx(Ui.dp(context, 76) + Ui.dp(context, 6)));
        TextView name = Ui.styledLabel(context, "AI 设计皮肤", Ui.TEXT_ROW_SUBTITLE + 1, 500,
            Ui.accent(context));
        ViewPolicy.setCentered(name);
        LinearLayout.LayoutParams nameParams = Ui.matchWidth();
        nameParams.topMargin = Ui.dp(context, 8);
        cell.addView(name, nameParams);
        cell.setAccessibilityDelegate(KeyboardSheets.buttonDelegate("AI 设计皮肤，描述一句话生成"));
        Ui.makeClickable(cell, context,
            () -> SettingsNavigator.open(requireContext(), PageId.AI_SKIN, null));
        return cell;
    }

    private void select(Card card) {
        HostTask.run(this, context -> apply(context, card), saved -> {
            if (saved == null) MsToast.show(requireContext(), "切换失败，保留当前皮肤");
            reload();
        });
    }

    @Nullable private static JSONObject apply(Context context, Card card) {
        JSONObject design = card.design();
        JSONObject saved = KeyboardSheets.write(context, preferences -> {
            if (design == null) preferences.put("global_theme", card.id());
            else KeyboardSheets.applyDesign(preferences, design);
        });
        if (saved == null) return null;
        if (design != null) KeyboardSheets.applyLocalFeedback(context, design);
        KeyboardSheets.recordSkin(context, card.id());
        return saved;
    }
}
