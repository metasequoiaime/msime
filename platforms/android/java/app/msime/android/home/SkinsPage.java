package app.msime.android.home;

import app.msime.android.ThemeColorPolicy;
import app.msime.android.LayoutPolicy;

import app.msime.android.DimensionPolicy;

import android.content.Context;
import android.os.Bundle;
import android.graphics.Color;
import android.graphics.drawable.GradientDrawable;
import android.view.View;
import android.view.ViewGroup;
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.annotation.Nullable;
import androidx.core.view.ViewCompat;
import app.msime.android.CustomKeyboardSkin;
import app.msime.android.CustomSkinLibrary;
import app.msime.android.DrawablePolicy;
import app.msime.android.KeyboardGeometry;
import app.msime.android.KeyboardSkin;
import app.msime.android.ViewPolicy;
import app.msime.android.R;
import app.msime.android.core.InputViewValuePolicy;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.List;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;
import com.google.android.material.tabs.TabLayout;

/**
 * 皮肤页：标题下固定的「内置 / 我的 / 社区」分类，分类内容使用两列预览卡，AI 设计入口放在「我的」。社区 Tab 直接承载完整列表，提供搜索、分类筛选、详情和举报。
 *
 * <p>社区分类复用嵌入模式的 {@link CommunityFragment}，不再展示缓存预览和更多入口；皮肤页仍经 {@link CommunitySkinCatalogue} 刷新键盘皮肤面板用的缓存。
 *
 * <p>点一张卡立刻生效：全局主题写 `global_theme`；我的设计按键盘自己的皮肤面板那样写进 `custom_theme.keyboard` 并选中 `custom`，同时按 P23 写设计带的按键动画和音效。每次换皮肤都在统计里记一次 `record_skin`，并标记云同步的「皮肤」分类。水杉四季那张卡跟着应用主题当前的季节写成「水杉四季 · 秋杉」。卡片预览用与键盘相同的解析器（{@link HostStore#keyboardSkin(JSONObject, boolean, app.msime.android.AppThemePalette.Seed)}），跟随系统那张按应用主题的种子色画。
 */
public final class SkinsPage extends ReloadingDetailPage {
    /** 本地皮肤卡：`design` 为 null 是全局主题，否则是皮肤库里的设计。 */
    private record Card(String id, String title, KeyboardSkin skin, @Nullable JSONObject design, boolean selected) {}

    private record Model(List<Card> themes, List<Card> designs) {}

    private static final String COMMUNITY_TAG = "skin-community";
    private boolean catalogueChecked;
    private int section;
    @Nullable private Model model;
    @Nullable private TabLayout tabs;
    @Nullable private FrameLayout communityContent;

    @Override public void onCreate(@Nullable Bundle state) {
        super.onCreate(state);
        if (state != null) section = Math.max(0, Math.min(2, state.getInt("skin-section", 0)));
    }

    @Override public void onSaveInstanceState(Bundle state) {
        super.onSaveInstanceState(state);
        state.putInt("skin-section", section);
    }

    @Override protected View fixedNavigation() {
        Context context = requireContext();
        tabs = new TabLayout(context);
        tabs.setTabMode(TabLayout.MODE_FIXED);
        tabs.setTabTextColors(ThemeColorPolicy.subText(context), ThemeColorPolicy.accent(context));
        tabs.setSelectedTabIndicatorColor(ThemeColorPolicy.accent(context));
        ViewPolicy.setBackgroundColor(tabs, ThemeColorPolicy.page(context));
        for (String title : new String[] {"内置", "我的", "社区"}) tabs.addTab(tabs.newTab().setText(title));
        tabs.selectTab(tabs.getTabAt(section));
        tabs.addOnTabSelectedListener(new TabLayout.OnTabSelectedListener() {
            @Override public void onTabSelected(TabLayout.Tab tab) {
                section = tab.getPosition();
                renderSection();
                scrollView().scrollTo(0, 0);
            }
            @Override public void onTabUnselected(TabLayout.Tab tab) {}
            @Override public void onTabReselected(TabLayout.Tab tab) {}
        });
        tabs.addOnLayoutChangeListener((view, left, top, right, bottom, oldLeft, oldTop, oldRight, oldBottom) ->
            positionCommunityContent());
        return tabs;
    }

    @Override protected void onContentReady() {
        communityContent = new FrameLayout(requireContext());
        communityContent.setId(R.id.skin_community_content);
        ((FrameLayout) requireView()).addView(communityContent, LayoutPolicy.frameMatchParentParams());
        positionCommunityContent();
        ViewPolicy.setVisible(communityContent, section == 2);
        if (section == 2) renderSection();
    }

    private void positionCommunityContent() {
        if (communityContent == null || tabs == null) return;
        FrameLayout.LayoutParams params = (FrameLayout.LayoutParams) communityContent.getLayoutParams();
        params.topMargin = DimensionPolicy.pixels(requireContext(), Ui.TOP_BAR_HEIGHT)
            + (tabs.getHeight() > 0 ? tabs.getHeight() : DimensionPolicy.pixels(requireContext(), 48));
        communityContent.setLayoutParams(params);
    }

    /** 社区列表独占分类下方空间，保留子 Fragment 的列表与搜索状态。 */
    private void showCommunity(boolean visible) {
        if (communityContent == null) return;
        ViewPolicy.setVisible(communityContent, visible);
        ViewPolicy.setVisible(scrollView(), !visible);
        CommunityFragment community = (CommunityFragment) getChildFragmentManager().findFragmentByTag(COMMUNITY_TAG);
        if (visible) {
            if (community == null && !getChildFragmentManager().isStateSaved()) {
                getChildFragmentManager().beginTransaction()
                    .add(R.id.skin_community_content, CommunityFragment.skinTab(), COMMUNITY_TAG)
                    .commitNow();
            } else if (community != null) community.refreshSkinLibrary();
        } else if (community != null) community.closeSearchKeyboard();
    }

    @Override protected void onContentDestroyed() {
        model = null;
        tabs = null;
        communityContent = null;
    }

    @Override protected void reload() {
        boolean dark = AppMode.dark(requireContext());
        String season = KeyboardSheets.seasonTitle(AppThemeController.cachedSeason(requireContext()));
        HostTask.run(this, context -> read(context, dark, season), this::render);
        // 缓存只服务键盘皮肤面板；社区 Tab 的搜索和分页由完整列表处理。
        if (!catalogueChecked) {
            catalogueChecked = true;
            CommunitySkinCatalogue.refresh(this, false, ignored -> { });
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
        this.model = model;
        renderSection();
    }

    private void renderSection() {
        LinearLayout target = contentColumn();
        if (target == null) return;
        target.removeAllViews();
        showCommunity(section == 2);
        if (section == 2) return;
        if (model == null) {
            GroupCard.add(target, null).note("读取设置失败。请先完成首次设置，或稍后返回重试。");
            return;
        }
        Context context = requireContext();
        if (section == 0) {
            List<View> cards = new ArrayList<>(model.themes().size());
            for (Card card : model.themes()) cards.add(card(context, card));
            grid(target, null, cards);
        } else if (section == 1) {
            List<View> cards = new ArrayList<>(model.designs().size() + 1);
            if (!AiSkinPage.hidden(context)) cards.add(aiCard(context));
            for (Card card : model.designs()) cards.add(card(context, card));
            if (cards.isEmpty()) GroupCard.add(target, null).note("还没有保存的皮肤。可到社区获取喜欢的皮肤。");
            else grid(target, null, cards);
        }
    }

    /** 一组两列卡片；组标题与其他详情页的组标题一样，卡片直接放在页面底色上。 */
    private static void grid(LinearLayout target, @Nullable String title, List<View> cards) {
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

        cell.setContentDescription("皮肤 " + card.title());
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
            }
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
