package app.msime.android.home;

import app.msime.android.ThemeColorPolicy;
import app.msime.android.LayoutPolicy;

import app.msime.android.DimensionPolicy;

import android.content.Context;
import android.graphics.Color;
import android.graphics.drawable.GradientDrawable;
import android.view.View;
import android.view.ViewGroup;
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.annotation.Nullable;
import androidx.core.view.ViewCompat;
import app.msime.android.CommunityCatalog;
import app.msime.android.CommunityRequest;
import app.msime.android.CommunitySkinCache;
import app.msime.android.CustomKeyboardSkin;
import app.msime.android.CustomSkinLibrary;
import app.msime.android.DrawablePolicy;
import app.msime.android.KeyboardGeometry;
import app.msime.android.KeyboardSkin;
import app.msime.android.ViewPolicy;
import app.msime.android.core.InputViewValuePolicy;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * 皮肤页：两列皮肤卡（预览加名字，选中的卡 2dp 强调色描边、名字前打 ✓），依次是共享目录里的全局主题、「我的设计」（自定义皮肤库，含社区下载和 AI 设计的皮肤），最后一张虚线卡进 AI 设计皮肤；再往下是「社区皮肤」，列出社区目录里还没存进皮肤库的前几款，末尾「浏览更多皮肤」进社区皮肤的完整列表（{@link CommunityFragment}），搜索、分类筛选、详情和举报都在那里。
 *
 * <p>社区皮肤读的是 {@link CommunitySkinCache}，和键盘的皮肤面板同一份；页面打开时缓存过期就经 {@link CommunitySkinCatalogue} 重拉一次，拉完再刷新这一组。点一张社区皮肤和在键盘皮肤面板里点一样：先存进皮肤库，再换上，之后它出现在「我的设计」里；同时在服务端记一次下载。
 *
 * <p>点一张卡立刻生效：全局主题写 `global_theme`；我的设计按键盘自己的皮肤面板那样写进 `custom_theme.keyboard` 并选中 `custom`，同时按 P23 写设计带的按键动画和音效。每次换皮肤都在统计里记一次 `record_skin`，并标记云同步的「皮肤」分类。水杉四季那张卡跟着应用主题当前的季节写成「水杉四季 · 秋杉」。卡片预览用与键盘相同的解析器（{@link HostStore#keyboardSkin(JSONObject, boolean, app.msime.android.AppThemePalette.Seed)}），跟随系统那张按应用主题的种子色画。
 */
public final class SkinsPage extends ReloadingDetailPage {
    /** 「社区皮肤」一组最多列几款，多的在完整列表里。 */
    private static final int COMMUNITY_LIMIT = 6;

    /** 一张卡：`design` 为 null 是全局主题，否则是皮肤库里的设计；`community` 为真时是社区目录里还没存进皮肤库的皮肤，`id` 是它的社区 id，`author` 是作者。 */
    private record Card(String id, String title, KeyboardSkin skin, @Nullable JSONObject design, boolean selected,
            boolean community, String author) {}

    private record Model(List<Card> themes, List<Card> designs, List<Card> community) {}

    /** 这一页对社区目录缓存做过什么：还没看过、正在重拉、看过了（新鲜或已重拉）、重拉失败。 */
    private enum Catalogue { UNCHECKED, LOADING, READY, FAILED }

    private Catalogue catalogue = Catalogue.UNCHECKED;

    @Override protected void reload() {
        boolean dark = AppMode.dark(requireContext());
        String season = KeyboardSheets.seasonTitle(AppThemeController.cachedSeason(requireContext()));
        HostTask.run(this, context -> read(context, dark, season), this::render);
        // 每次打开这一页最多看一次缓存新旧，过期就重拉；重拉完再读一遍，新的社区皮肤就出现在下面那一组里。
        if (catalogue == Catalogue.UNCHECKED) {
            catalogue = Catalogue.LOADING;
            CommunitySkinCatalogue.refresh(this, false, refreshed -> {
                catalogue = refreshed == null ? Catalogue.FAILED : Catalogue.READY;
                reload();
            });
        }
    }

    @Nullable private static Model read(Context context, boolean systemDark, String season) {
        JSONObject preferences = KeyboardSheets.preferences(context);
        if (preferences == null) return null;
        JSONArray catalog = HostStore.themeCatalog();
        // 键盘的明暗，与 HostStore.keyboardSkin 的算法相同。只列这一明暗下的主题；所选主题属于另一种明暗时键盘按跟随系统画，选中态也落在跟随系统上。
        boolean keyboardDark = KeyboardSkin.resolveDark(
            InputViewValuePolicy.textOr(preferences, "screen_keyboard_theme", "follow"),
            InputViewValuePolicy.textOr(preferences, "theme", "system"), systemDark);
        String current = KeyboardSkin.effectiveTheme(catalog,
            InputViewValuePolicy.textOr(preferences, "global_theme", "system"), keyboardDark);
        List<Card> themes = new ArrayList<>(catalog.length());
        try {
            for (int index = 0; index < catalog.length(); index++) {
                JSONObject entry = catalog.optJSONObject(index);
                if (entry == null) continue;
                String id = InputViewValuePolicy.textOr(entry, "id", "");
                // 「自定义」由下面的我的设计代替：它画的就是当前选中的那个设计。
                if (id.isEmpty() || "custom".equals(id)) continue;
                if (!KeyboardSkin.fitsMode(KeyboardSkin.themeAppearance(catalog, id), keyboardDark)) continue;
                String title = InputViewValuePolicy.textOr(entry, "title", id);
                if (entry.optBoolean("seasonal", false)) title = title + " · " + season;
                JSONObject probe = new JSONObject(preferences.toString()).put("global_theme", id);
                KeyboardSkin skin = HostStore.keyboardSkin(probe, systemDark, HostStore.seed(context));
                themes.add(new Card(id, title, skin, null, id.equals(current), false, ""));
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
                        item.design(), selected, false, ""));
                }
            } catch (java.io.IOException error) {
                // 皮肤库读不出来时只少了我的设计这一组，内置主题照常显示。
                android.util.Log.w("MSIMESettings", "Custom skin library unreadable", error);
            }
        }
        return new Model(themes, designs, community(directory, designs, storedKey, systemDark));
    }

    /** 社区目录缓存里还没存进皮肤库的前几款；与皮肤库里某个设计画出来一样的也不列，和键盘皮肤面板的去重相同。 */
    private static List<Card> community(String directory, List<Card> designs, String storedKey, boolean systemDark) {
        if (directory.isEmpty()) return List.of();
        Set<String> ids = new HashSet<>(designs.size());
        Set<String> keys = new HashSet<>(designs.size());
        for (Card design : designs) {
            ids.add(design.id());
            if (design.design() != null) keys.add(CustomKeyboardSkin.from(design.design()).key());
        }
        List<Card> cards = new ArrayList<>(COMMUNITY_LIMIT);
        for (CommunitySkinCache.Entry entry : CommunitySkinCache.read(Paths.get(directory))) {
            if (cards.size() >= COMMUNITY_LIMIT) break;
            if (ids.contains(entry.id())) continue;
            String key = CustomKeyboardSkin.from(entry.design()).key();
            if (!keys.add(key)) continue;
            boolean selected = !storedKey.isEmpty() && storedKey.equals(key);
            cards.add(new Card(entry.id(), entry.name(), KeyboardSkin.custom(entry.design(), systemDark),
                entry.design(), selected, true, entry.author()));
        }
        return cards;
    }

    private void render(@Nullable Model model) {
        LinearLayout target = contentColumn();
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
        renderCommunity(target, context, model.community());
    }

    /** 「社区皮肤」：缓存里的前几款，末尾一行进完整列表。缓存是空的时候说明为什么空。 */
    private void renderCommunity(LinearLayout target, Context context, List<Card> cards) {
        if (!cards.isEmpty()) {
            List<View> communityCards = new ArrayList<>(cards.size());
            for (Card card : cards) communityCards.add(card(context, card));
            grid(target, "社区皮肤", communityCards);
        }
        GroupCard more = GroupCard.add(target, cards.isEmpty() ? "社区皮肤" : null);
        if (cards.isEmpty()) {
            more.note(switch (catalogue) {
                case UNCHECKED, LOADING -> "正在读取社区皮肤…";
                case FAILED -> "暂时连不上社区，稍后再试。";
                case READY -> "社区里暂时没有你还没获取的皮肤。";
            });
        }
        more.nav("浏览更多皮肤", "搜索、按分类筛选、看详情", null,
            () -> CommunityFragment.browse(requireContext(), CommunityRequest.Kind.SKIN));
    }

    /** 一组两列卡片；组标题与其他详情页的组标题一样，卡片直接放在页面底色上。 */
    private static void grid(LinearLayout target, String title, List<View> cards) {
        GroupCard group = GroupCard.add(target, title);
        LinearLayout holder = group.card();
        ViewPolicy.clearBackground(holder);
        holder.setClipToOutline(false);
        Context context = target.getContext();
        for (int start = 0; start < cards.size(); start += 2) {
            LinearLayout row = LayoutPolicy.row(context);
            row.setBaselineAligned(false);
            for (int slot = 0; slot < 2; slot++) {
                int index = start + slot;
                View cell = index < cards.size() ? cards.get(index) : new View(context);
                LinearLayout.LayoutParams params = LayoutPolicy.weightedWrapParams(1f);
                if (slot == 1) params.setMarginStart(DimensionPolicy.pixels(context, 12));
                row.addView(cell, params);
            }
            LinearLayout.LayoutParams rowParams = LayoutPolicy.matchWidthWrapParams();
            if (start > 0) rowParams.topMargin = DimensionPolicy.pixels(context, 14);
            holder.addView(row, rowParams);
        }
    }

    private View card(Context context, Card card) {
        LinearLayout cell = LayoutPolicy.column(context);
        ViewPolicy.setCenteredHorizontally(cell);

        FrameLayout tile = new FrameLayout(context);
        int ring = DimensionPolicy.pixels(context, 2);
        GradientDrawable frame = DrawablePolicy.outlined(Color.TRANSPARENT, DimensionPolicy.pixels(context, 14),
            card.selected() ? ring : DimensionPolicy.atLeastOnePixel(context, 1),
            card.selected() ? ThemeColorPolicy.accent(context) : ThemeColorPolicy.outline(context));
        ViewPolicy.setBackground(tile, frame);
        int inset = ring + DimensionPolicy.pixels(context, 1);
        ViewPolicy.setPadding(tile, inset, inset, inset, inset);
        SkinSwatchView swatch = new SkinSwatchView(context);
        swatch.setSkin(card.skin());
        ViewPolicy.hideFromAccessibility(swatch);
        tile.addView(swatch,
            LayoutPolicy.frameMatchWidthHeightPx(DimensionPolicy.pixels(context, 76)));
        cell.addView(tile, LayoutPolicy.matchWidthWrapParams());

        TextView name = ViewPolicy.centeredLabel(context, card.selected() ? "✓ " + card.title() : card.title(),
            Ui.TEXT_ROW_SUBTITLE + 1, card.selected() ? 600 : 400,
            card.selected() ? ThemeColorPolicy.accent(context) : ThemeColorPolicy.text(context));
        ViewPolicy.setSingleLineEllipsized(name);
        LinearLayout.LayoutParams nameParams = LayoutPolicy.matchWidthWrapParams();
        nameParams.topMargin = DimensionPolicy.pixels(context, 8);
        cell.addView(name, nameParams);

        cell.setContentDescription((card.community() ? "社区皮肤 " : "皮肤 ") + card.title());
        ViewCompat.setStateDescription(cell, card.selected() ? "已选中" : "未选中");
        ViewPolicy.makeClickable(cell, context, () -> {
            if (!card.selected()) select(card);
        });
        return cell;
    }

    /** 虚线卡：「✦ 描述一句话生成」，名字是「AI 设计皮肤」，点了进 AI 设计页。 */
    private View aiCard(Context context) {
        LinearLayout cell = LayoutPolicy.column(context);
        ViewPolicy.setCenteredHorizontally(cell);
        LinearLayout tile = LayoutPolicy.column(context);
        ViewPolicy.setCentered(tile);
        GradientDrawable dashed = DrawablePolicy.outlinedDashed(ThemeColorPolicy.accentSoft(context), DimensionPolicy.pixels(context, 14),
            DimensionPolicy.atLeastOnePixel(context, 1.5f), ThemeColorPolicy.accent(context), DimensionPolicy.pixels(context, 6),
            DimensionPolicy.pixels(context, 4));
        ViewPolicy.setBackground(tile, dashed);
        TextView spark = ViewPolicy.centeredLabel(context, "✦", 22, 400, ThemeColorPolicy.accent(context));
        tile.addView(spark);
        TextView hint = ViewPolicy.centeredLabel(context, "描述一句话生成", 12, 400, ThemeColorPolicy.accent(context));
        tile.addView(hint);
        cell.addView(tile, LayoutPolicy.matchWidthHeightPx(DimensionPolicy.pixels(context, 76) + DimensionPolicy.pixels(context, 6)));
        TextView name = ViewPolicy.centeredLabel(context, "AI 设计皮肤", Ui.TEXT_ROW_SUBTITLE + 1, 500,
            ThemeColorPolicy.accent(context));
        LinearLayout.LayoutParams nameParams = LayoutPolicy.matchWidthWrapParams();
        nameParams.topMargin = DimensionPolicy.pixels(context, 8);
        cell.addView(name, nameParams);
        cell.setAccessibilityDelegate(HomeButtonPolicy.buttonDelegate("AI 设计皮肤，描述一句话生成"));
        ViewPolicy.makeClickable(cell, context,
            () -> SettingsNavigator.open(requireContext(), PageId.AI_SKIN, null));
        return cell;
    }

    private void select(Card card) {
        HostTask.run(this, context -> apply(context, card), saved -> {
            if (saved == null) {
                MsToast.show(requireContext(), "切换失败，保留当前皮肤");
            } else if (card.community()) {
                MsToast.show(requireContext(), "已换上「" + card.title() + "」");
                recordDownload(card);
            }
            reload();
        });
    }

    /** 和社区列表里的「获取」一样在服务端记一次下载；尽力而为，记不上只是计数少一次。 */
    private void recordDownload(Card card) {
        CommunityCatalog.Item item = new CommunityCatalog.Item(card.id(), CommunityRequest.Kind.SKIN, card.title(),
            "", card.author(), 0, 0, 0.0, card.design(), CommunityRequest.Category.OTHER, false, 0, null);
        HostTask.runNetwork(this, context -> new CommunityCatalog(context).recordDownload(item), ignored -> { });
    }

    @Nullable private static JSONObject apply(Context context, Card card) {
        JSONObject design = card.design();
        // 社区皮肤先存进皮肤库（键盘皮肤面板读的同一份），再换上；皮肤库满了照样换上，只是不进「我的设计」，与键盘皮肤面板相同。
        if (card.community() && design != null) {
            String directory = HostStore.directory(context);
            try {
                if (!directory.isEmpty()
                        && !CustomSkinLibrary.add(Paths.get(directory), card.id(), card.title(), design)) {
                    android.util.Log.i("MSIMESettings", "Skin library is full; the community skin is applied without being saved");
                }
            } catch (java.io.IOException | RuntimeException error) {
                android.util.Log.w("MSIMESettings", "Community skin was not saved to the library", error);
            }
        }
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
