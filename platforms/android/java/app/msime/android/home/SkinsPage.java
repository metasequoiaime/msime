package app.msime.android.home;

import app.msime.android.ThemeColorPolicy;
import app.msime.android.LayoutPolicy;

import app.msime.android.DimensionPolicy;

import android.content.Context;
import android.os.Bundle;
import android.view.View;
import android.view.LayoutInflater;
import android.widget.FrameLayout;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.annotation.Nullable;
import androidx.core.view.ViewCompat;
import app.msime.android.CustomKeyboardSkin;
import app.msime.android.CustomSkinLibrary;
import app.msime.android.DrawablePolicy;
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

    private record Model(List<Card> themes, List<Card> designs, boolean nineKey) {}

    private static final String COMMUNITY_TAG = "skin-community";
    private boolean catalogueChecked;
    private int section;
    @Nullable private Model model;
    @Nullable private FrameLayout navigation;
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
        navigation = new FrameLayout(context);
        ViewPolicy.setSymmetricPaddingDp(context, navigation, Ui.PAGE_PADDING, 4);
        ViewPolicy.setBackgroundColor(navigation, ThemeColorPolicy.page(context));
        SegmentedControl tabs = new SegmentedControl(context);
        tabs.setFillWidth(true);
        tabs.setMinimumHeight(DimensionPolicy.pixels(context, 40));
        ViewPolicy.setBackground(tabs, DrawablePolicy.pill(ThemeColorPolicy.card(context)));
        tabs.setOptions(List.of("内置", "我的", "社区"), section);
        tabs.setOnSelect(index -> {
            section = index;
            renderSection();
            scrollView().scrollTo(0, 0);
        });
        navigation.addView(tabs, LayoutPolicy.frameMatchWidthWrapParams());
        navigation.addOnLayoutChangeListener((view, left, top, right, bottom, oldLeft, oldTop, oldRight, oldBottom) ->
            positionCommunityContent());
        return navigation;
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
        if (communityContent == null || navigation == null) return;
        FrameLayout.LayoutParams params = (FrameLayout.LayoutParams) communityContent.getLayoutParams();
        params.topMargin = DimensionPolicy.pixels(requireContext(), Ui.TOP_BAR_HEIGHT)
            + (navigation.getHeight() > 0 ? navigation.getHeight() : DimensionPolicy.pixels(requireContext(), 48));
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
            } else if (community != null) community.refreshTakenMarks();
        } else if (community != null) community.closeSearchKeyboard();
    }

    @Override protected void onContentDestroyed() {
        model = null;
        navigation = null;
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
        return new Model(themes, designs, "nine_key".equals(
            InputViewValuePolicy.textOr(preferences, "touch_keyboard_layout", "twenty_six_key")));
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
            grid(target, cards);
        } else if (section == 1) {
            List<View> cards = new ArrayList<>(model.designs().size() + 1);
            if (!AiSkinPage.hidden(context)) cards.add(aiCard(context));
            for (Card card : model.designs()) cards.add(card(context, card));
            if (cards.isEmpty()) GroupCard.add(target, null).note("还没有保存的皮肤。可到社区获取喜欢的皮肤。");
            else grid(target, cards);
        }
    }

    /** 两列卡片的边距与社区网格一致：页面缩进 10dp，每张卡片四周留 6dp。 */
    private static void grid(LinearLayout target, List<View> cards) {
        Context context = target.getContext();
        ViewPolicy.setPaddingDp(context, target, 10, 10, 10, 0);
        for (int start = 0; start < cards.size(); start += 2) {
            LinearLayout row = LayoutPolicy.row(context);
            row.setBaselineAligned(false);
            for (int slot = 0; slot < 2; slot++) {
                int index = start + slot;
                View cell = index < cards.size() ? cards.get(index) : new View(context);
                LinearLayout.LayoutParams params = LayoutPolicy.weightedWrapParams(1f);
                int margin = DimensionPolicy.pixels(context, 6);
                params.setMargins(margin, margin, margin, margin);
                row.addView(cell, params);
            }
            target.addView(row, LayoutPolicy.matchWidthWrapParams());
        }
    }

    /** 本地皮肤复用社区卡片布局，只展示自己的名称和选择状态。 */
    private View localCard(Context context) {
        View cell = LayoutInflater.from(context).inflate(R.layout.item_community_skin, null, false);
        for (int id : new int[] {R.id.community_item_badge, R.id.community_item_category,
                R.id.community_item_author, R.id.community_item_rating, R.id.community_item_action}) {
            ViewPolicy.hide(cell.findViewById(id));
        }
        return cell;
    }

    private View card(Context context, Card card) {
        View cell = localCard(context);
        KeyboardPreview preview = cell.findViewById(R.id.community_item_preview);
        preview.setCornerRadiusDp(5f);
        preview.setKeyboard(card.skin(), model != null && model.nineKey());
        ViewPolicy.show(preview);

        TextView name = cell.findViewById(R.id.community_item_name);
        name.setText(card.selected() ? "✓ " + card.title() : card.title());
        ViewPolicy.setTextColor(name, card.selected() ? ThemeColorPolicy.accent(context) : ThemeColorPolicy.text(context));
        cell.setContentDescription("皮肤 " + card.title());
        ViewCompat.setStateDescription(cell, card.selected() ? "已选中" : "未选中");
        ViewPolicy.bindClick(cell, () -> {
            if (!card.selected()) select(card);
        });
        return cell;
    }

    /** AI 入口与皮肤卡同尺寸，尚未生成设计时用说明替代键盘预览。 */
    private View aiCard(Context context) {
        View cell = localCard(context);
        KeyboardPreview preview = cell.findViewById(R.id.community_item_preview);
        ViewPolicy.setInvisible(preview);
        FrameLayout frame = (FrameLayout) preview.getParent();
        LinearLayout tile = LayoutPolicy.column(context);
        ViewPolicy.setCentered(tile);
        ViewPolicy.setBackground(tile, DrawablePolicy.rounded(ThemeColorPolicy.accentSoft(context),
            DimensionPolicy.pixels(context, 5)));
        tile.addView(ViewPolicy.centeredLabel(context, "✦", 28, 400, ThemeColorPolicy.accent(context)));
        tile.addView(ViewPolicy.centeredLabel(context, "描述一句话生成", 12, 400, ThemeColorPolicy.accent(context)));
        frame.addView(tile, LayoutPolicy.frameMatchParentParams());
        ((TextView) cell.findViewById(R.id.community_item_name)).setText("AI 设计皮肤");
        cell.setAccessibilityDelegate(HomeButtonPolicy.buttonDelegate("AI 设计皮肤，描述一句话生成"));
        ViewPolicy.bindClick(cell, () -> SettingsNavigator.open(requireContext(), PageId.AI_SKIN, null));
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
