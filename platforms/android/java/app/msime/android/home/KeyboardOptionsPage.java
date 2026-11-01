package app.msime.android.home;

import app.msime.android.ClipboardExport;
import app.msime.android.ClipboardHistoryPolicy;
import app.msime.android.CandidateScalePolicy;
import app.msime.android.CandidatePreeditStylePolicy;

import app.msime.android.DimensionPolicy;
import app.msime.android.KeyboardBottomBarPolicy;
import app.msime.android.KeyboardFormFactorPolicy;

import app.msime.android.ThemeColorPolicy;
import app.msime.android.ToolbarOrderPolicy;
import app.msime.android.ToolbarSizePolicy;
import app.msime.android.KeyboardSpacingPolicy;
import app.msime.android.KeyboardHeightPolicy;
import app.msime.android.KeyboardTypographyPolicy;
import app.msime.android.NumberPolicy;

import app.msime.android.LayoutPolicy;

import android.content.Context;
import android.graphics.Color;
import android.view.View;
import android.view.ViewGroup;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.annotation.Nullable;
import app.msime.android.AndroidLocalSettings;
import app.msime.android.DrawablePolicy;
import app.msime.android.AppEdition;
import app.msime.android.ClipboardLayoutPolicy;
import app.msime.android.ClipboardUndoPolicy;
import app.msime.android.ClipboardAuthenticationActivity;
import app.msime.android.ClipboardDeleteAuthenticationPolicy;
import app.msime.android.ColorPolicy;
import app.msime.android.KeyboardFeedbackPreferences;
import app.msime.android.KeyboardFeedbackStore;
import app.msime.android.KeyboardScheme;
import app.msime.android.KeyboardSkin;
import app.msime.android.NineKeySwipePolicy;
import app.msime.android.OneHandSidePadPolicy;
import app.msime.android.QuanpinSeparatorPolicy;
import app.msime.android.RecentClipboardSuggestion;
import app.msime.android.SchemePreferences;
import app.msime.android.ShuangpinKeyHintPolicy;
import app.msime.android.SplitKeyboardPolicy;
import app.msime.android.SwipeHintPolicy;
import app.msime.android.SyncSignals;
import app.msime.android.SyncSwitch;
import app.msime.android.TouchKeyStyle;
import app.msime.android.KeyLayoutStyleMigration;
import app.msime.android.TextTrackpadSettings;
import app.msime.android.ViewPolicy;
import app.msime.android.core.InputViewValuePolicy;
import app.msime.android.JsonPolicy;
import app.msime.android.policy.KeySoundPackPolicy;
import org.json.JSONObject;

/**
 * 键盘页：布局（中文键盘 26 / 9 键，双拼时是双拼的 26 / 14 / 18 键，五笔时是「五笔键盘」的 26 / 14 键、按键布局子页（三类键面的底行样式、九键与数字键盘的左侧符号和顶部数字行，{@link KeyLayoutStylesPage}）、双拼键位提示、键盘高度、按键间距、行间距、分离式键盘、竖屏也分离、浮动键盘、键盘底栏、键盘底部高度）、按键反馈（按键音、音效、按键振动、按键弹出预览、按键动画）、手势（滑动输入符号及其方向、滑行输入、空格滑动移动光标、长按空格语音）、工具栏分页里的键盘工具栏（预览、显示方式、各按钮）与剪贴板（工具栏显示最近复制、排列）和子页「AI 润色与回复」。
 *
 * <p>键盘与本页读同一批存储：按键间距、行间距、竖屏分离、双拼键位提示、全拼组字保留 Shift、九键滑动输入数字和表情/剪贴板/皮肤三个工具栏按钮在共享偏好里（`touch_key_spacing_tenths`、`touch_row_spacing_tenths`、`touch_keyboard_split_portrait`、`touch_shuangpin_key_hints`、`touch_quanpin_shift`、`touch_nine_key_swipe_digits`、`touch_toolbar.*`）；键盘高度、分离式键盘主开关、手机横屏分离式键盘、浮动键盘、物理键盘时候选栏跟随光标、键盘底栏、底部留白、按键弹出预览、按键动画、三个手势、常用语/输入方式/浮动键盘三个工具栏按钮和「显示方式：隐藏」（整行不显示，候选条照常显示）只有 Android 用，在 {@link AndroidLocalSettings} 里。键盘高度按设计以 75–170 % 显示，存的是 dp（{@link KeyboardHeightPolicy#heightPercentToAdjustment}），本地没写过时沿用共享偏好里旧的 `touch_keyboard_height_adjustment`。按键音和按键振动是 Android 一直以来的本地开关（`KeyboardFeedbackStore`），键盘的功能面板改的也是它们；按键音用哪个音效包在共享偏好 `plugins.key_sound.pack` 里（{@link KeySoundPacks}）。
 */
public class KeyboardOptionsPage extends ReloadingDetailPage {
    private final ClipboardExportActions clipboardExport = new ClipboardExportActions(this, this::reload);
    private boolean clipboardDeleteAuthenticationPending;

    @Override public void onCreate(@Nullable android.os.Bundle saved) {
        super.onCreate(saved);
        clipboardExport.restore(saved);
    }

    @Override public void onSaveInstanceState(android.os.Bundle saved) {
        clipboardExport.save(saved);
        super.onSaveInstanceState(saved);
    }

    @Override protected void onContentDestroyed() { clipboardExport.destroyed(); }
    @Override public void onDestroy() {
        clipboardDeleteAuthenticationPending = false;
        super.onDestroy();
    }
    @Override protected java.util.List<String> sectionTitles() {
        return java.util.List.of("布局", "反馈", "手势", "工具栏");
    }

    private static final String[] ANIMATIONS = {"bounce", "ripple", "glow", "lift", "none"};
    private static final String[] ANIMATION_LABELS = {"弹起", "涟漪", "发光", "浮起", "无"};
    private record State(JSONObject preferences, AndroidLocalSettings.Snapshot local,
                         KeyboardFeedbackStore.Settings feedback, KeyboardSkin skin, String exportDirectory,
                         JSONObject trackpadOptions, java.util.List<KeySoundPackPolicy.Pack> soundPacks) {}

    @Override protected void reload() {
        boolean dark = AppMode.dark(requireContext());
        HostTask.run(this, context -> {
            // 先把本机旧的长空格设置迁进共享偏好，工具栏预览按迁移后的底行样式画顶部符号入口。
            KeyLayoutStyleMigration.migrate(context, HostStore.directory(context));
            JSONObject preferences = KeyboardSheets.preferences(context);
            if (preferences == null) return null;
            return new State(preferences, AndroidLocalSettings.load(context), KeyboardFeedbackStore.load(context),
                HostStore.keyboardSkin(context, preferences, dark, HostStore.seed(context)), ClipboardExport.directoryName(context),
                HostStore.textTrackpadOptions(), KeySoundPacks.load(context));
        }, this::render);
    }

    private void render(@Nullable State state) {
        LinearLayout target = contentColumn();
        if (target == null) return;
        target.removeAllViews();
        if (state == null) {
            GroupCard.add(target, null).note("读取设置失败。请先完成首次设置，或稍后返回重试。");
            return;
        }
        JSONObject preferences = state.preferences();
        AndroidLocalSettings.Snapshot settings = state.local();
        AppEdition edition = AppEdition.current();
        KeyboardScheme current = SchemePreferences.storedScheme(preferences, edition);

        GroupCard layout = GroupCard.add(sectionColumn(target, "布局"), "布局");
        layout.toggle("英文自动大写", null,
            settings.bool(AndroidLocalSettings.AUTO_CAPITALIZATION),
            checked -> saveLocalSetting(AndroidLocalSettings.AUTO_CAPITALIZATION, checked)).help("按输入框要求自动大写句首、单词首字母或全部字母；默认关闭，需要大写时按 Shift");
        layout.toggle("交换语言键与数字键", null,
            settings.bool(AndroidLocalSettings.SWAP_LANGUAGE_NUMBER_KEYS),
            checked -> saveLocalSetting(AndroidLocalSettings.SWAP_LANGUAGE_NUMBER_KEYS, checked)).help("语言键放到左侧，数字键移到原语言键位置；英文和外语键盘一起调整");
        if (current.shuangpinProfile() != null) {
            // 双拼的排法单独存在 `touch_shuangpin_layout` 里，换排法不换双拼方案。它原先在输入页双拼面板的第四层；这里与 iOS、鸿蒙一样放在键盘页。
            String shuangpinLayout = InputViewValuePolicy.textOr(preferences, "touch_shuangpin_layout", "twenty_six_key");
            layout.nav("中文键盘", null, shuangpinLayoutLabel(shuangpinLayout),
                () -> pickShuangpinLayout(shuangpinLayout)).help("保留当前双拼方案，只换键盘排法");
        } else if (current == KeyboardScheme.WUBI) {
            // 五笔的排法单独存在 `touch_wubi_layout` 里，与双拼的做法相同：换排法不换五笔方案，也不动 86/98（`wubi_profile`）。行名与 iOS、鸿蒙的「五笔键盘」一致。
            String wubiLayout = InputViewValuePolicy.textOr(preferences, "touch_wubi_layout", "twenty_six_key");
            layout.nav("五笔键盘", null, wubiLayoutLabel(wubiLayout),
                () -> pickWubiLayout(wubiLayout)).help("保留当前五笔版本，只换键盘排法，86 和 98 五笔共用");
        } else {
            KeyboardScheme[] pair = layoutPair(current);
            boolean pairOffered = pair != null;
            if (pair != null) for (KeyboardScheme choice : pair) pairOffered &= choice.offeredBy(edition);
            String note = pairOffered ? null
                : pair == null ? "当前方案只有一种键盘，在「输入」里换方案" : "本版本只有一种键盘";
            layout.nav("中文键盘", note, layoutLabel(current),
                pairOffered ? () -> pickLayout(current, pair) : null);
        }
        // 底行样式与顶部数字行在共享偏好里，三端相同；子页按本版本提供的键面列出几组，一组都没有（例如日语版）时不列这个入口，与鸿蒙同一条规则。说明三端相同。
        if (TouchKeyStyle.Groups.offeredBy(edition).any())
            layout.nav("按键布局", null, null,
                () -> SettingsNavigator.open(requireContext(), PageId.KEY_LAYOUT_STYLES, null)).help("各类键盘的底行样式、键位和顶部数字行");
        String spellingPosition = CandidatePreeditStylePolicy.normalize(
            InputViewValuePolicy.textOr(preferences, "touch_spelling_position", null));
        layout.nav("拼音显示位置", null,
            spellingPositionLabel(spellingPosition),
            () -> pickSpellingPosition(spellingPosition)).help("全拼、双拼、五笔和粤拼可以把正在输入的拼音放在输入框或候选栏");
        // 本版本没有任何其他语言键盘（五笔版、拼音版）时不列这一项。
        if (offersOtherLanguage(edition)) {
            layout.toggle("中英键轮换其他语言", null,
                settings.bool(AndroidLocalSettings.LANGUAGE_KEY_CYCLE),
                checked -> saveLocalSetting(AndroidLocalSettings.LANGUAGE_KEY_CYCLE, checked)).help("中英键依次切换中文、英文和已添加的日语、韩语等键盘，再回到中文；没有添加其他语言时仍只切中英");
        }
        layout.nav("文本编辑按键顺序", null,
            null, () -> new TextEditOrderSheet(this, preferences, this::reload).show()).help("调整文本编辑面板两侧的七个功能键，方向键与换行位置固定");
        layout.nav("符号分类顺序", null,
            null, () -> new SymbolCategoryOrderSheet(this, settings, this::reload).show()).help("调整符号面板底部分类的排列顺序");
        // 只有双拼方案的 26 键画键位提示；本版本没有双拼时不列这一行。缺省为关，与键盘和 client-core 一致。
        if (KeyboardScheme.XIAOHE.offeredBy(edition)) {
            layout.toggle("双拼键位提示", null,
                preferences.optBoolean(ShuangpinKeyHintPolicy.PREFERENCE_KEY, false),
                checked -> savePreference(ShuangpinKeyHintPolicy.PREFERENCE_KEY, checked)).help("双拼方案的 26 键键盘在字母键底部显示这个键代表的声母和韵母；关闭后键面只留字母");
        }
        layout.toggle("组字时保留 Shift", null,
            preferences.optBoolean(QuanpinSeparatorPolicy.PREFERENCE_KEY, true),
            checked -> savePreference(QuanpinSeparatorPolicy.PREFERENCE_KEY, checked)).help("全拼 26 键组字时左下角保留 ⇧，点一下再输字母即辅助码；默认开启，关闭后这一键改为「分词」，用来断开音节");
        int height = KeyboardHeightPolicy.heightAdjustmentToPercent(
            settings.has(AndroidLocalSettings.KEYBOARD_HEIGHT_ADJUSTMENT)
                ? settings.integer(AndroidLocalSettings.KEYBOARD_HEIGHT_ADJUSTMENT)
                : NumberPolicy.strictInt(preferences, "touch_keyboard_height_adjustment", Integer.MIN_VALUE));
        layout.slider("键盘高度", KeyboardHeightPolicy.MIN_HEIGHT_PERCENT, KeyboardHeightPolicy.MAX_HEIGHT_PERCENT, 1, height,
            KeyboardHeightPolicy::displayPercent,
            percent -> saveLocalSetting(AndroidLocalSettings.KEYBOARD_HEIGHT_ADJUSTMENT,
                KeyboardHeightPolicy.heightPercentToAdjustment(percent)));
        layout.slider("按键间距", KeyboardSpacingPolicy.MIN_KEY_SPACING_TENTHS, KeyboardSpacingPolicy.MAX_KEY_SPACING_TENTHS, 1,
            KeyboardSpacingPolicy.keySpacing(NumberPolicy.strictInt(preferences, "touch_key_spacing_tenths", -1)),
            KeyboardSpacingPolicy::display, value -> savePreference("touch_key_spacing_tenths", value));
        layout.slider("行间距", KeyboardSpacingPolicy.MIN_ROW_SPACING_TENTHS, KeyboardSpacingPolicy.MAX_ROW_SPACING_TENTHS, 1,
            KeyboardSpacingPolicy.rowSpacing(NumberPolicy.strictInt(preferences, "touch_row_spacing_tenths", -1)),
            KeyboardSpacingPolicy::display, value -> savePreference("touch_row_spacing_tenths", value));
        boolean showLargeScreenOptions = KeyboardFormFactorPolicy.showLargeScreenOptions(
            requireContext().getResources().getConfiguration().smallestScreenWidthDp);
        if (showLargeScreenOptions) {
            layout.toggle("分离式键盘", null,
                settings.bool(AndroidLocalSettings.SPLIT_KEYBOARD),
                checked -> saveLocalSetting(AndroidLocalSettings.SPLIT_KEYBOARD, checked)).help("默认仅在大屏横屏时生效：26 键和韩文键盘分成左右两半，方便双手握持时用拇指输入；右半边也有 G 和 V");
            layout.toggle("竖屏也分离", null,
                preferences.optBoolean(SplitKeyboardPolicy.PORTRAIT_PREFERENCE, false),
                checked -> savePreference(SplitKeyboardPolicy.PORTRAIT_PREFERENCE, checked)).help("平板或折叠屏内屏竖放时也分离；回到窄外屏后自动使用完整键盘")
                .setEnabled(settings.bool(AndroidLocalSettings.SPLIT_KEYBOARD));
            boolean splitEnabled = settings.bool(AndroidLocalSettings.SPLIT_KEYBOARD);
            layout.slider("分离启用宽度", SplitKeyboardPolicy.MIN_THRESHOLD_DP, SplitKeyboardPolicy.MAX_THRESHOLD_DP,
                SplitKeyboardPolicy.THRESHOLD_STEP_DP, settings.integer(AndroidLocalSettings.SPLIT_KEYBOARD_THRESHOLD),
                value -> value + " dp", value -> saveLocalSetting(AndroidLocalSettings.SPLIT_KEYBOARD_THRESHOLD, value))
                .setEnabled(splitEnabled);
            layout.slider("分离中间空隙", SplitKeyboardPolicy.MIN_GAP_PERCENT, SplitKeyboardPolicy.MAX_GAP_PERCENT,
                SplitKeyboardPolicy.GAP_PERCENT_STEP, settings.integer(AndroidLocalSettings.SPLIT_KEYBOARD_GAP),
                value -> value + "%", value -> saveLocalSetting(AndroidLocalSettings.SPLIT_KEYBOARD_GAP, value))
                .setEnabled(splitEnabled);
            // #10203：大屏单手模式空出来的一侧放数字键盘。写共享偏好（随账号同步），三端标题和说明逐字相同；不认识的取值（以后的文本编辑面板）按关显示。
            boolean sidePad = OneHandSidePadPolicy.parse(JsonPolicy.strictString(
                preferences.opt(OneHandSidePadPolicy.PREFERENCE_KEY))) == OneHandSidePadPolicy.Panel.NUMBER_PAD;
            layout.toggle("大屏单手时侧边显示数字键盘", null,
                sidePad, checked -> savePreference(OneHandSidePadPolicy.PREFERENCE_KEY,
                    (checked ? OneHandSidePadPolicy.Panel.NUMBER_PAD : OneHandSidePadPolicy.Panel.OFF).wire)).help("单手模式下用九键打字时，空出来的一侧显示数字键盘。只在平板、折叠屏展开和 iPad 全宽键盘上生效，分离式和浮动键盘不变。");
        } else {
            // #10440：手机用独立的本机开关，不随账号同步，免得继承平板上的分离主开关。
            layout.toggle("手机横屏分离式键盘", null,
                settings.bool(AndroidLocalSettings.SPLIT_KEYBOARD_PHONE_LANDSCAPE),
                checked -> saveLocalSetting(AndroidLocalSettings.SPLIT_KEYBOARD_PHONE_LANDSCAPE, checked)).help("开启后仅在横屏时生效：26 键和韩文键盘分成左右两半，方便双手握持时用拇指输入；按键盘实际可用宽度判断，分开后按键太窄时（例如分屏）仍用完整键盘");
        }
        GroupCard.Row[] bottomHeight = new GroupCard.Row[1];
        layout.toggle("浮动键盘", null,
            settings.bool(AndroidLocalSettings.FLOATING_KEYBOARD),
            checked -> {
                saveLocalSetting(AndroidLocalSettings.FLOATING_KEYBOARD, checked);
                if (bottomHeight[0] != null) bottomHeight[0].setEnabled(!checked);
            }).help("键盘缩小成一块面板悬在应用上面，按住顶部横条拖动位置；也可以在功能面板或工具栏按钮里切换");
        if (showLargeScreenOptions) {
            layout.toggle("物理键盘时候选栏跟随光标", null,
                settings.bool(AndroidLocalSettings.HARDWARE_CARET_CANDIDATES),
                checked -> saveLocalSetting(AndroidLocalSettings.HARDWARE_CARET_CANDIDATES, checked)).help("接实体键盘打字时，候选栏显示在输入光标旁，靠近屏幕底部时显示在光标上方；不打字时不显示工具栏。应用不提供光标位置时仍停在屏幕底部");
        }
        layout.toggle("键盘底栏", null,
            settings.bool(AndroidLocalSettings.BOTTOM_BAR),
            checked -> saveLocalSetting(AndroidLocalSettings.BOTTOM_BAR, checked)).help("仅在全面屏手势导航下生效：键盘下方加一条栏，左边切换输入法、右边剪贴板，中间左右滑动移动光标，同时把按键抬离屏幕底边");
        // #10447：只有一个「目标总距离」滑块，底栏画着时它的 40 dp 计在里面；沿用 bottom_padding 和 bottom_padding_height 两个旧字段，拖到 0 只关开关、保留原高度。
        // 浮动键盘时运行时不垫高（extraPaddingDp），滑块置灰；手机横屏和外接键盘收起键区是键盘当时的状态，设置页判断不了，写进说明。
        bottomHeight[0] = layout.slider("键盘底部高度", null,
            KeyboardBottomBarPolicy.MIN_PADDING_DP, KeyboardBottomBarPolicy.MAX_PADDING_DP, KeyboardBottomBarPolicy.PADDING_STEP_DP,
            AndroidLocalSettings.bottomHeightDp(settings), value -> value + " dp",
            value -> saveLocalSettings(AndroidLocalSettings.bottomHeightEdits(value))).help("抬高整块键盘，按键大小不变。底栏显示时，40 dp 以内由底栏占用；浮动键盘、手机横屏和接实体键盘收起键区时不生效。");
        bottomHeight[0].setEnabled(!settings.bool(AndroidLocalSettings.FLOATING_KEYBOARD));

        GroupCard feedback = GroupCard.add(sectionColumn(target, "反馈"), "按键反馈");
        KeyboardFeedbackStore.Settings local = state.feedback();
        GroupCard.Row[] soundPackRow = new GroupCard.Row[1];
        feedback.toggle("按键音", null, local.soundEnabled(), checked -> {
            saveFeedback(checked, null);
            soundPackRow[0].setEnabled(checked);
        });
        // 按键音包在共享偏好里（随设置同步），开关在本机：关着时这一行置灰，选了也听不到。
        JSONObject plugins = preferences.optJSONObject("plugins");
        JSONObject keySound = plugins == null ? null : plugins.optJSONObject("key_sound");
        String soundPack = KeySoundPackPolicy.selected(keySound == null ? null
            : JsonPolicy.strictStringOrEmpty(keySound.opt("pack")));
        java.util.List<KeySoundPackPolicy.Pack> soundPacks = state.soundPacks();
        soundPackRow[0] = feedback.nav("音效", null, KeySoundPackPolicy.label(soundPacks, soundPack),
            () -> pickSoundPack(soundPacks, soundPackRow[0]));
        soundPackRow[0].setEnabled(local.soundEnabled());
        feedback.toggle("按键振动", null, local.hapticsEnabled(), checked -> saveFeedback(null, checked));
        feedback.toggle("按键弹出预览", null,
            settings.bool(AndroidLocalSettings.KEY_POPUP), checked -> saveLocalSetting(AndroidLocalSettings.KEY_POPUP, checked)).help("按下时在键上方显示放大的字符");
        String animation = settings.choice(AndroidLocalSettings.KEY_ANIMATION);
        GroupCard.Row[] animationRow = new GroupCard.Row[1];
        animationRow[0] = feedback.nav("按键动画", null, animationLabel(animation),
            () -> pickAnimation(animationRow[0]));

        GroupCard gestures = GroupCard.add(sectionColumn(target, "手势"), "手势");
        boolean swipeSymbols = settings.bool(AndroidLocalSettings.SWIPE_DOWN_SYMBOLS);
        gestures.toggle("滑动输入符号", null, swipeSymbols,
            checked -> KeyboardSheets.saveLocal(this, AndroidLocalSettings.SWIPE_DOWN_SYMBOLS, checked,
                this::reload, this::reload)).help("沿设定方向滑动输入角标，中文时反方向打开字母与符号选项；关闭后仍可长按");
        String swipeDirection = settings.choice(AndroidLocalSettings.SWIPE_SYMBOLS_DIRECTION);
        GroupCard.Row[] directionRow = new GroupCard.Row[1];
        directionRow[0] = gestures.nav("滑动方向", null,
            swipeDirectionLabel(swipeDirection), () -> pickSwipeDirection(swipeDirection, directionRow[0])).help("该方向直接输入角标，中文时反方向打开选项；关闭滑动输入符号后两种滑动都不生效");
        directionRow[0].setEnabled(swipeSymbols);
        // 九键的滑动单独一项、默认关闭（#5580）：九键原来没有滑动，和 26 键共用上面那个默认开着的开关会改变老用户的点按。它在共享偏好 `touch_nine_key_swipe_digits` 里、随账号同步，三端同一行、同一套文字（#10229）；本机旧值在 reload 里先迁过来。本版本没有拼音九键时不列。
        boolean quanpinNineKey = KeyboardScheme.QUANPIN_NINE_KEY.offeredBy(edition);
        if (quanpinNineKey) {
            String nineKeySwipe = NineKeySwipePolicy.mode(
                JsonPolicy.strictString(preferences.opt(NineKeySwipePolicy.PREFERENCE)));
            GroupCard.Row[] nineKeySwipeRow = new GroupCard.Row[1];
            nineKeySwipeRow[0] = gestures.nav("九键滑动输入数字", null,
                nineKeySwipeLabel(nineKeySwipe), () -> pickNineKeySwipe(nineKeySwipe, nineKeySwipeRow[0])).help("沿所选方向滑动输入数字，反方向弹出字母。");
            // 九键单独的判定距离（#6208）：键高，快速连打时手指往上带一下就会被当成上滑；26 键的「滑动输入符号」阈值不受影响。距离仍是 Android 本机设置，iOS 和鸿蒙固定 24，不随账号同步。
            GroupCard.Row distanceRow = gestures.slider("九键滑动距离", null,
                NineKeySwipePolicy.MIN_THRESHOLD_DP, NineKeySwipePolicy.MAX_THRESHOLD_DP,
                NineKeySwipePolicy.THRESHOLD_STEP_DP, settings.integer(AndroidLocalSettings.NINE_KEY_SWIPE_DISTANCE),
                value -> value + " dp", value -> saveLocalSetting(AndroidLocalSettings.NINE_KEY_SWIPE_DISTANCE, value)).help("手指移动超过这个距离才会触发；距离越大越不容易把快速连打误判成滑动");
            distanceRow.setEnabled(!NineKeySwipePolicy.OFF.equals(nineKeySwipe));
        }
        String quickDeleteScope = settings.choice(AndroidLocalSettings.QUICK_DELETE_SCOPE);
        GroupCard.Row[] quickDeleteRow = new GroupCard.Row[1];
        quickDeleteRow[0] = gestures.nav("快速删除范围", null,
            quickDeleteScopeLabel(quickDeleteScope), () -> pickQuickDeleteScope(quickDeleteScope, quickDeleteRow[0])).help("上滑退格松手时删除的范围；普通退格和左滑连续删除不受影响");
        gestures.toggle("滑行输入", null,
            settings.bool(AndroidLocalSettings.GLIDE_TYPING),
            checked -> saveLocalSetting(AndroidLocalSettings.GLIDE_TYPING, checked)).help("在 26 键上连续滑过拼音的字母，抬手出词；在键上稍作停留可确认经过的键");
        gestures.toggle("空格键滑动移动光标", null, settings.bool(AndroidLocalSettings.SPACE_CURSOR),
            checked -> saveLocalSetting(AndroidLocalSettings.SPACE_CURSOR, checked));
        gestures.toggle("长按空格语音输入", null, settings.bool(AndroidLocalSettings.SPACE_VOICE),
            checked -> saveLocalSetting(AndroidLocalSettings.SPACE_VOICE, checked));
        gestures.help("退格键左滑可连续删除，保持按住再右滑可逐步恢复，松手后无法恢复；密码、无痕和禁止学习的输入框不支持此手势。上滑仍可快速删除，有备份时长按下滑可撤回。");
        gestures.help("空格键上滑后松手可复制当前输入框的全文，滑回可取消；复制预览遵守下方剪贴板设置。长按语音开始后，上滑仍用于取消语音。");
        renderTrackpad(sectionColumn(target, "手势"), preferences, state.trackpadOptions());

        boolean hidden = settings.bool(AndroidLocalSettings.TOOLBAR_HIDDEN);
        GroupCard bar = GroupCard.add(sectionColumn(target, "工具栏"), "键盘工具栏");
        int candidateScale = CandidateScalePolicy.percent(
            NumberPolicy.strictInt(preferences, "candidate_scale_percent", CandidateScalePolicy.DEFAULT_PERCENT));
        bar.addView(toolbarPreview(requireContext(), preferences, settings, state.skin(),
            TouchKeyStyle.Row.LONG_SPACE.wire.equals(JsonPolicy.strictString(
                preferences.opt(TouchKeyStyle.TWENTY_SIX_KEY_PREFERENCE))), candidateScale));
        bar.nav("显示方式", null, hidden ? "隐藏" : "输入时显示", () -> pickToolbarMode(hidden));
        // 共享偏好 `candidate_scale_percent`，与 iOS、鸿蒙同一范围和说法（#10137）：候选栏和跟随它的空闲工具栏一起变矮或变高，按键不动。工具栏隐藏时候选栏照样缩放，所以这一行不跟着置灰。
        bar.slider("候选栏大小", null,
            CandidateScalePolicy.MIN_PERCENT, CandidateScalePolicy.MAX_PERCENT, CandidateScalePolicy.STEP_PERCENT,
            candidateScale, value -> value + "%", this::saveCandidateScale).help("候选字和工具栏一起缩放，按键大小不变");
        boolean customToolbarSize = settings.has(AndroidLocalSettings.TOOLBAR_SIZE_PERCENT);
        int toolbarSize = settings.integer(AndroidLocalSettings.TOOLBAR_SIZE_PERCENT);
        bar.nav("工具栏尺寸", null,
            customToolbarSize ? toolbarSize + "%" : "跟随候选栏",
            hidden ? null : () -> pickToolbarSize(customToolbarSize, toolbarSize)).help("跟随候选栏时随「候选栏大小」缩放；单独设定后只调整工具栏的图标和按钮，较矮的一侧在上方留白，按键不跳动");
        bar.nav("按钮顺序", null, null,
            hidden ? null : () -> new ToolbarOrderSheet(this, settings, this::reload).show()).help("调整工具按钮的排列；品牌和收起键保留在两端");
        for (ToolbarOrderPolicy.Item button : toolbarOrder(settings)) {
            String member = button.preferenceKey;
            String description = button == ToolbarOrderPolicy.Item.UNDO || button == ToolbarOrderPolicy.Item.REDO
                ? "由当前应用处理；部分输入框不支持，密码框不可用"
                : button == ToolbarOrderPolicy.Item.VOICE ? VOICE_TOGGLE_DESCRIPTION
                : button == ToolbarOrderPolicy.Item.RANDOM_DIGITS
                    ? "在中文工具栏显示“随”开关；点按后打乱数字九宫格，每输入一个数字重新排列" : null;
            GroupCard.Row row = bar.toggle(button.label, null, toolbarButton(preferences, settings, member),
                checked -> saveToolbar(member, checked)).help(description);
            row.setEnabled(!hidden);
        }

        // 剪贴板面板从工具栏打开，它的设置和键盘工具栏放在同一个分页，页面最多四个分页。
        GroupCard clipboard = GroupCard.add(sectionColumn(target, "工具栏"), "剪贴板");
        clipboard.headerAction(clipboardExport.button());
        clipboard.nav("导出文件夹", null, state.exportDirectory(), clipboardExport::choose).help("全文保存为 TXT；自选目录只在本机生效");
        // 剪贴板历史关着时键盘不显示最近复制，也不为它读剪贴板（RecentClipboardSuggestion.enabled），这一项跟着置灰。
        boolean clipboardHistory = preferences.optBoolean("clipboard_history", false);
        // 复制后快捷键（#10079）挂在最近复制那一行上：最近复制关着时置灰，快捷键总开关关着时功能单选也置灰。开关切换后保存成功不重画页面，所以由这里直接更新两行的置灰状态。
        boolean[] quickActionState = {settings.bool(AndroidLocalSettings.CLIPBOARD_SUGGESTION),
            settings.bool(AndroidLocalSettings.CLIPBOARD_SUGGESTION_ACTION)};
        GroupCard.Row[] quickActionRows = new GroupCard.Row[2];
        Runnable syncQuickActionRows = () -> {
            if (quickActionRows[0] != null) quickActionRows[0].setEnabled(clipboardHistory && quickActionState[0]);
            if (quickActionRows[1] != null)
                quickActionRows[1].setEnabled(clipboardHistory && quickActionState[0] && quickActionState[1]);
        };
        GroupCard.Row recentClipRow = clipboard.toggle("工具栏显示最近复制", null,
            quickActionState[0],
            checked -> {
                quickActionState[0] = checked;
                syncQuickActionRows.run();
                saveLocalSetting(AndroidLocalSettings.CLIPBOARD_SUGGESTION, checked);
            }).help("复制文字或图片后一分钟内，键盘顶部显示这条内容：文字点按即可粘贴，图片在支持收图片的输入框里点按即可发送；需先在「隐私」里打开剪贴板历史。隐私模式、密码框和系统标为敏感的内容不显示");
        recentClipRow.setEnabled(clipboardHistory);
        quickActionRows[0] = clipboard.toggle("复制后自动显示快捷键", null,
            quickActionState[1],
            checked -> {
                quickActionState[1] = checked;
                syncQuickActionRows.run();
                saveLocalSetting(AndroidLocalSettings.CLIPBOARD_SUGGESTION_ACTION, checked);
            }).help("最近复制那一行的预览右边多一个快捷键，点按直接进入这条内容的分词或移动分类；复制的是图片时不显示");
        String quickActionKind = settings.choice(AndroidLocalSettings.CLIPBOARD_SUGGESTION_ACTION_KIND);
        quickActionRows[1] = clipboard.nav("快捷键功能", null,
            RecentClipboardSuggestion.quickActionLabel(quickActionKind),
            () -> pickClipboardQuickAction(quickActionKind, quickActionRows[1])).help("点按快捷键时进入分词点选，还是移动到分类");
        syncQuickActionRows.run();
        String columns = settings.choice(AndroidLocalSettings.CLIPBOARD_COLUMNS);
        clipboard.nav("排列", null, clipboardColumnsLabel(columns),
            () -> pickClipboardColumns(columns)).help("剪贴板面板里一行显示几条记录");
        // 删除剪贴板记录（单条或多选）后可撤销的秒数（#10073、#10080），只影响之后的删除；键盘每次删除时读这一项。
        int undoSeconds = settings.integer(AndroidLocalSettings.CLIPBOARD_UNDO_SECONDS);
        clipboard.nav("撤销时长", null,
            ClipboardUndoPolicy.settingLabel(undoSeconds), () -> pickClipboardUndoSeconds(undoSeconds)).help("删除记录后，剪贴板面板右下角的「撤销」按钮保留几秒");
        boolean deleteAuthentication = settings.bool(AndroidLocalSettings.CLIPBOARD_DELETE_AUTHENTICATION);
        GroupCard.Row[] deleteAuthenticationRow = new GroupCard.Row[1];
        deleteAuthenticationRow[0] = clipboard.toggle("剪贴板删除数据验证", null,
            deleteAuthentication,
            checked -> requestClipboardDeleteAuthentication(deleteAuthenticationRow[0], deleteAuthentication, checked)).help("单条或多选删除前要求指纹、图案或密码等系统认证；访客剪贴板始终要求认证，撤销删除不再认证");
        clipboard.nav("剪贴板分类", null, "管理",
            () -> SettingsNavigator.open(requireContext(), PageId.CLIPBOARD_CATEGORIES, null)).help("自定义分类名称，在键盘中手动移动记录");
        int historyLimit = ClipboardHistoryPolicy.limit(preferences);
        clipboard.nav("历史记录上限", null,
            historyLimit + " 条", () -> pickClipboardLimit(historyLimit)).help("达到上限后，最旧的未固定记录会被新记录替换");

    }

    /** 切换删除门禁也必须先认证；取消或失败时把开关留在原状态，不写入本地设置。设备没有可用的系统认证（桥返回 2002）时按 {@link ClipboardDeleteAuthenticationPolicy#togglesWithoutAuthenticator} 只放行关闭：打开开关后移除了锁屏凭据的用户否则永远关不掉它。 */
    private void requestClipboardDeleteAuthentication(GroupCard.Row row, boolean current, boolean next) {
        row.setChecked(current);
        if (clipboardDeleteAuthenticationPending) return;
        clipboardDeleteAuthenticationPending = true;
        row.setEnabled(false);
        android.os.ResultReceiver receiver = new android.os.ResultReceiver(
            new android.os.Handler(android.os.Looper.getMainLooper())) {
            @Override protected void onReceiveResult(int code, android.os.Bundle ignored) {
                clipboardDeleteAuthenticationPending = false;
                if (getView() == null) return;
                if (code == ClipboardAuthenticationActivity.RESULT_AUTHENTICATED) {
                    saveLocalSetting(AndroidLocalSettings.CLIPBOARD_DELETE_AUTHENTICATION, next);
                } else if (code == ClipboardAuthenticationActivity.RESULT_UNAVAILABLE
                        && ClipboardDeleteAuthenticationPolicy.togglesWithoutAuthenticator(next)) {
                    saveLocalSetting(AndroidLocalSettings.CLIPBOARD_DELETE_AUTHENTICATION, next);
                    MsToast.show(requireContext(), "这台设备没有可用的系统认证，已直接关闭剪贴板删除数据验证");
                } else {
                    row.setEnabled(true);
                    if (code == ClipboardAuthenticationActivity.RESULT_UNAVAILABLE)
                        MsToast.show(requireContext(), "这台设备没有可用的系统认证，请先设置指纹、图案或密码");
                }
            }
        };
        try {
            ClipboardAuthenticationActivity.launch(requireContext(), receiver, "修改剪贴板删除验证");
        } catch (RuntimeException unavailable) {
            clipboardDeleteAuthenticationPending = false;
            row.setEnabled(true);
            MsToast.show(requireContext(), "这台设备没有可用的系统认证，请先设置指纹、图案或密码");
        }
    }

    /** 工具栏按钮独立控制，不改变长按空格语音；两入口可同时使用。 */
    static final String VOICE_TOGGLE_DESCRIPTION = "点按开始语音输入；不影响长按空格语音，可同时使用";

    private static java.util.List<ToolbarOrderPolicy.Item> toolbarOrder(AndroidLocalSettings.Snapshot settings) {
        return ToolbarOrderPolicy.ordered(settings.text(AndroidLocalSettings.TOOLBAR_ORDER));
    }

    /** 工具栏按钮是否开启：本地项读取本地设置，语音读共享偏好顶层的 `touch_voice_shortcut`（缺键为关），其余读取 `touch_toolbar`；撤销、重做缺键为关。 */
    private static boolean toolbarButton(JSONObject preferences, AndroidLocalSettings.Snapshot settings, String member) {
        if (AndroidLocalSettings.specs().containsKey(member)) return settings.bool(member);
        if (member.equals(ToolbarOrderPolicy.VOICE_SHORTCUT_PREFERENCE)) return preferences.optBoolean(member, false);
        JSONObject toolbar = preferences.optJSONObject("touch_toolbar");
        if (toolbar == null) return !member.equals("undo") && !member.equals("redo");
        return toolbar.optBoolean(member, !member.equals("undo") && !member.equals("redo"));
    }

    /** 工具栏预览：用当前皮肤的底色和图标色，按开关列出会出现的按钮；隐藏时说明只剩候选条。26 键选了长空格样式时符号入口在工具栏上，预览也画出来。跟随候选栏时预览高度和字号随「候选栏大小」变化，与键盘一样不高于 44 dp 的按钮高。 */
    private static View toolbarPreview(Context context, JSONObject preferences, AndroidLocalSettings.Snapshot settings,
                                       KeyboardSkin skin, boolean longSpace, int candidateScale) {
        LinearLayout strip = LayoutPolicy.row(context);
        ViewPolicy.setCenteredVertically(strip);
        int pad = DimensionPolicy.pixels(context, 12);
        ViewPolicy.setSymmetricPadding(strip, pad);
        LinearLayout plate = LayoutPolicy.row(context);
        ViewPolicy.setCenteredVertically(plate);
        ViewPolicy.setHorizontalPaddingDp(context, plate, 10);
        ViewPolicy.setBackground(plate, DrawablePolicy.rounded(ColorPolicy.parse(skin.background(), ThemeColorPolicy.page(context)), DimensionPolicy.pixels(context, 12)));
        ViewPolicy.setImportantForAccessibility(plate,
            View.IMPORTANT_FOR_ACCESSIBILITY_NO_HIDE_DESCENDANTS);
        int icon = ColorPolicy.parse(skin.toolbarIcon(), ThemeColorPolicy.subText(context));
        boolean customSize = settings.has(AndroidLocalSettings.TOOLBAR_SIZE_PERCENT);
        int sizePercent = customSize ? settings.integer(AndroidLocalSettings.TOOLBAR_SIZE_PERCENT) : 100;
        int buttonDp = ToolbarSizePolicy.buttonHeightDp(sizePercent);
        int rowDp = ToolbarSizePolicy.followedHeightDp(candidateScale);
        int designRowDp = ToolbarSizePolicy.followedHeightDp(CandidateScalePolicy.DEFAULT_PERCENT);
        int previewDp = customSize ? buttonDp : ToolbarSizePolicy.followedButtonHeight(rowDp, buttonDp, designRowDp);
        int chipPercent = customSize ? sizePercent : ToolbarSizePolicy.followedPercent(rowDp, buttonDp, designRowDp);
        boolean anyButton = false;
        for (ToolbarOrderPolicy.Item button : ToolbarOrderPolicy.DEFAULT_ORDER)
            anyButton |= toolbarButton(preferences, settings, button.preferenceKey);
        boolean hidden = settings.bool(AndroidLocalSettings.TOOLBAR_HIDDEN);
        if (hidden && !longSpace) {
            TextView note = ViewPolicy.styledLabel(context, "工具栏已隐藏，只显示候选条", 13, 400, icon);
            plate.addView(note);
        } else if (!anyButton && !longSpace) {
            // 键盘在工具按钮全关时不画这一行（MSIMEInputService.toolbarRowHidden），预览跟着说明。
            TextView note = ViewPolicy.styledLabel(context, "没有打开的工具按钮，工具栏不显示，只显示候选条", 13, 400, icon);
            plate.addView(note);
        } else {
            addChip(context, plate, "水杉", ColorPolicy.parse(skin.accentText(), ThemeColorPolicy.accent(context)), chipPercent);
            if (longSpace) addChip(context, plate, "符", icon, chipPercent);
            if (!hidden) for (ToolbarOrderPolicy.Item button : toolbarOrder(settings))
                if (toolbarButton(preferences, settings, button.preferenceKey)) addChip(context, plate, button == ToolbarOrderPolicy.Item.RANDOM_DIGITS ? "随" : button.label, icon, chipPercent);
        }
        strip.addView(plate, LayoutPolicy.matchWidthHeightDp(context, previewDp));
        strip.setContentDescription("工具栏预览");
        return strip;
    }

    private static void addChip(Context context, LinearLayout plate, String label, int colour, int sizePercent) {
        TextView chip = ViewPolicy.styledLabel(context, label, Math.round(12f * sizePercent / 100f), 500, colour);
        ViewPolicy.setSingleLine(chip);
        LinearLayout.LayoutParams params = LayoutPolicy.wrapParams();
        params.setMarginEnd(DimensionPolicy.pixels(context, 12));
        plate.addView(chip, params);
    }

    /**
     * 当前方案可选的几种排法，按 26 键、14 键、9 键排列：全拼是全拼 26 键、全拼 14 键和全拼 9 键（三选一，选中即启用并切换），注音是大千和注音 9 键，日语是日语 26 键和日语 9 键（14 键只给全拼）。双拼的排法另存在 `touch_shuangpin_layout`，由 `pickShuangpinLayout` 选；五笔的 26 / 14 键另存在 `touch_wubi_layout`，由 `pickWubiLayout` 选；手写这类只有一种排法的方案没有这一组，返回 null：原来一律给全拼的那一对，小鹤双拼用户在这里点哪一项都会被改成全拼，注音 9 键用户点「26 键」会切到全拼而不是大千。
     */
    @Nullable
    private static boolean offersOtherLanguage(AppEdition edition) {
        for (KeyboardScheme scheme : KeyboardScheme.values()) {
            if (scheme.otherLanguage() && scheme.offeredBy(edition)) return true;
        }
        return false;
    }

    private static KeyboardScheme[] layoutPair(KeyboardScheme current) {
        return switch (current) {
            case QUANPIN, QUANPIN_FOURTEEN_KEY, QUANPIN_EIGHTEEN_KEY, QUANPIN_NINE_KEY -> new KeyboardScheme[] {KeyboardScheme.QUANPIN,
                KeyboardScheme.QUANPIN_EIGHTEEN_KEY, KeyboardScheme.QUANPIN_FOURTEEN_KEY, KeyboardScheme.QUANPIN_NINE_KEY};
            case ZHUYIN, ZHUYIN_NINE_KEY -> new KeyboardScheme[] {KeyboardScheme.ZHUYIN, KeyboardScheme.ZHUYIN_NINE_KEY};
            case JAPANESE, JAPANESE_NINE_KEY -> new KeyboardScheme[] {KeyboardScheme.JAPANESE, KeyboardScheme.JAPANESE_NINE_KEY};
            default -> null;
        };
    }

    private void pickLayout(KeyboardScheme current, KeyboardScheme[] choices) {
        OptionSheet sheet = new OptionSheet(requireContext(), "中文键盘", null);
        // 点已选中的那一项什么也不做：OptionSheet 对选中项也会执行动作，重写一遍方案没有意义。
        for (KeyboardScheme choice : choices)
            sheet.option(layoutLabel(choice), choice == current, () -> { if (choice != current) applyScheme(choice); });
        sheet.show();
    }

    /** 双拼「中文键盘」的 26、14、18 键。 */
    private static String shuangpinLayoutLabel(String layout) {
        return switch (layout) {
            case "fourteen_key" -> "14 键";
            case "eighteen_key" -> "18 键";
            default -> "26 键";
        };
    }

    private void pickShuangpinLayout(String current) {
        OptionSheet sheet = new OptionSheet(requireContext(), "中文键盘", "保留当前双拼方案");
        // 点已选中的那一项什么也不做，与 `pickLayout` 相同。
        for (String option : java.util.List.of("twenty_six_key", "fourteen_key", "eighteen_key"))
            sheet.option(shuangpinLayoutLabel(option), option.equals(current),
                () -> { if (!option.equals(current)) savePreference("touch_shuangpin_layout", option); });
        sheet.show();
    }

    /** 「五笔键盘」的 26、14 键。 */
    private static String wubiLayoutLabel(String layout) {
        return "fourteen_key".equals(layout) ? "14 键" : "26 键";
    }

    private void pickWubiLayout(String current) {
        OptionSheet sheet = new OptionSheet(requireContext(), "五笔键盘", "保留当前五笔版本");
        // 点已选中的那一项什么也不做，与 `pickLayout` 相同。
        for (String option : java.util.List.of("twenty_six_key", "fourteen_key"))
            sheet.option(wubiLayoutLabel(option), option.equals(current),
                () -> { if (!option.equals(current)) savePreference("touch_wubi_layout", option); });
        sheet.show();
    }

    /** 「中文键盘」按入口存下的触屏布局显示九、十四、十八或二十六键。 */
    private static String layoutLabel(KeyboardScheme scheme) {
        return switch (scheme.touchKeyboardLayout()) {
            case "nine_key" -> "9 键";
            case "fourteen_key" -> "14 键";
            case "eighteen_key" -> "18 键";
            default -> "26 键";
        };
    }

    private static String spellingPositionLabel(String value) {
        return CandidatePreeditStylePolicy.keepsSpellingOnKeyboard(value) ? "候选栏" : "输入框";
    }

    private void pickSpellingPosition(String current) {
        boolean candidateBar = CandidatePreeditStylePolicy.keepsSpellingOnKeyboard(current);
        OptionSheet sheet = new OptionSheet(requireContext(), "拼音显示位置", null);
        sheet.option("输入框", !candidateBar,
            () -> savePreference("touch_spelling_position", CandidatePreeditStylePolicy.EDITOR));
        sheet.option("候选栏", candidateBar,
            () -> savePreference("touch_spelling_position", CandidatePreeditStylePolicy.CANDIDATE_BAR));
        sheet.show();
    }

    /** 当前方案的布局经 {@link SchemePreferences#withScheme} 与输入页、引导页按同一契约保存。 */
    private void applyScheme(KeyboardScheme scheme) {
        HostTask.run(this, context -> {
            JSONObject snapshot = HostStore.loadPreferences(context);
            JSONObject pending = SchemePreferences.withScheme(snapshot, scheme, null);
            return pending == null ? null : HostStore.savePreferences(context, pending);
        }, saved -> {
            if (saved == null) MsToast.show(requireContext(), "保存失败，键盘保留当前布局");
            reload();
        });
    }

    private void pickAnimation(GroupCard.Row row) {
        HostTask.run(this, context -> AndroidLocalSettings.load(context).choice(AndroidLocalSettings.KEY_ANIMATION), selected -> {
            if (selected == null) {
                MsToast.show(requireContext(), "读取设置失败，请重试");
                return;
            }
            OptionSheet sheet = new OptionSheet(requireContext(), "按键动画", null);
            for (int index = 0; index < ANIMATIONS.length; index++) {
                String value = ANIMATIONS[index];
                String label = ANIMATION_LABELS[index];
                sheet.option(label, value.equals(selected), () -> {
                    row.setValue(label);
                    saveLocalSetting(AndroidLocalSettings.KEY_ANIMATION, value);
                });
            }
            sheet.show();
        });
    }

    /** 选按键音包：写进共享偏好 `plugins.key_sound.pack`，并用选中的包试听一声。当前值在打开时重读，免得别处刚改过。 */
    private void pickSoundPack(java.util.List<KeySoundPackPolicy.Pack> packs, GroupCard.Row row) {
        HostTask.run(this, KeyboardSheets::preferences, preferences -> {
            if (preferences == null) {
                MsToast.show(requireContext(), "读取设置失败，请重试");
                return;
            }
            JSONObject plugins = preferences.optJSONObject("plugins");
            JSONObject keySound = plugins == null ? null : plugins.optJSONObject("key_sound");
            String selected = KeySoundPackPolicy.selected(keySound == null ? null
                : JsonPolicy.strictStringOrEmpty(keySound.opt("pack")));
            Object volume = keySound == null ? null : keySound.opt("volume");
            OptionSheet sheet = new OptionSheet(requireContext(), "音效", null);
            for (KeySoundPackPolicy.Pack pack : packs) {
                sheet.option(pack.name(), pack.id().equals(selected), () -> {
                    row.setValue(pack.name());
                    HostTask.run(this, context -> {
                        KeySoundPacks.preview(context, pack.id(), volume);
                        return Boolean.TRUE;
                    }, ignored -> { });
                    if (!pack.id().equals(selected)) savePreferences(edited -> KeyboardSheets.child(
                        KeyboardSheets.child(edited, "plugins"), "key_sound").put("pack", pack.id()));
                });
            }
            sheet.show();
        });
    }

    private void pickSwipeDirection(String selected, GroupCard.Row row) {
        OptionSheet sheet = new OptionSheet(requireContext(), "滑动方向", null);
        for (String value : new String[] {SwipeHintPolicy.DOWN, SwipeHintPolicy.UP}) {
            sheet.option(swipeDirectionLabel(value), value.equals(selected),
                () -> {
                    row.setValue(swipeDirectionLabel(value));
                    KeyboardSheets.saveLocal(this, AndroidLocalSettings.SWIPE_SYMBOLS_DIRECTION, value,
                        this::reload, this::reload);
                });
        }
        sheet.show();
    }

    private void pickQuickDeleteScope(String selected, GroupCard.Row row) {
        OptionSheet sheet = new OptionSheet(requireContext(), "快速删除范围", null);
        for (String value : new String[] {"before", "all"}) {
            sheet.option(quickDeleteScopeLabel(value), value.equals(selected), () -> {
                row.setValue(quickDeleteScopeLabel(value));
                KeyboardSheets.saveLocal(this, AndroidLocalSettings.QUICK_DELETE_SCOPE, value,
                    this::reload, this::reload);
            });
        }
        sheet.show();
    }

    private static String quickDeleteScopeLabel(String value) {
        return "all".equals(value) ? "整个输入框" : "光标前文字";
    }

    private void pickClipboardColumns(String selected) {
        OptionSheet sheet = new OptionSheet(requireContext(), "剪贴板排列", null);
        for (String value : new String[] {ClipboardLayoutPolicy.ONE_COLUMN, ClipboardLayoutPolicy.TWO_COLUMNS}) {
            sheet.option(clipboardColumnsLabel(value), value.equals(selected),
                () -> KeyboardSheets.saveLocal(this, AndroidLocalSettings.CLIPBOARD_COLUMNS, value,
                    this::reload, this::reload));
        }
        sheet.show();
    }

    private void pickClipboardUndoSeconds(int selected) {
        OptionSheet sheet = new OptionSheet(requireContext(), "撤销时长", null);
        for (int seconds = ClipboardUndoPolicy.MIN_SECONDS; seconds <= ClipboardUndoPolicy.MAX_SECONDS; seconds++) {
            int value = seconds;
            sheet.option(ClipboardUndoPolicy.settingLabel(value), value == selected,
                () -> KeyboardSheets.saveLocal(this, AndroidLocalSettings.CLIPBOARD_UNDO_SECONDS, value,
                    this::reload, this::reload));
        }
        sheet.show();
    }

    private void pickClipboardQuickAction(String selected, GroupCard.Row row) {
        OptionSheet sheet = new OptionSheet(requireContext(), "快捷键功能", null);
        for (String value : new String[] {RecentClipboardSuggestion.ACTION_SEGMENT,
                RecentClipboardSuggestion.ACTION_CATEGORY}) {
            String label = RecentClipboardSuggestion.quickActionLabel(value);
            sheet.option(label, value.equals(selected), () -> {
                row.setValue(label);
                KeyboardSheets.saveLocal(this, AndroidLocalSettings.CLIPBOARD_SUGGESTION_ACTION_KIND, value,
                    this::reload, this::reload);
            });
        }
        sheet.show();
    }

    private void pickClipboardLimit(int selected) {
        OptionSheet sheet = new OptionSheet(requireContext(), "历史记录上限", null);
        for (int limit : ClipboardHistoryPolicy.LIMITS) {
            sheet.option(limit + " 条", limit == selected,
                () -> savePreference("clipboard_history_limit", limit));
        }
        sheet.show();
    }

    private static String clipboardColumnsLabel(String value) {
        return ClipboardLayoutPolicy.columns(value) == 2 ? "双列" : "单列";
    }

    private void pickNineKeySwipe(String selected, GroupCard.Row row) {
        OptionSheet sheet = new OptionSheet(requireContext(), "九键滑动输入数字", null);
        for (String value : new String[] {NineKeySwipePolicy.OFF, SwipeHintPolicy.UP, SwipeHintPolicy.DOWN}) {
            sheet.option(nineKeySwipeLabel(value), value.equals(selected), () -> {
                row.setValue(nineKeySwipeLabel(value));
                // 选哪项写哪项的写法（关闭也写 `off`），写进共享偏好、不写本机键；存好后重画，下次打开选项单时勾选的是新值，距离滑块跟着置灰或恢复。
                savePreference(NineKeySwipePolicy.PREFERENCE, value);
            });
        }
        sheet.show();
    }

    /** 行值和选项单三项共用，三端逐字相同。 */
    private static String nineKeySwipeLabel(String value) {
        return SwipeHintPolicy.UP.equals(value) ? "上滑" : SwipeHintPolicy.DOWN.equals(value) ? "下滑" : "关闭";
    }

    private static String swipeDirectionLabel(String value) {
        return SwipeHintPolicy.UP.equals(value) ? "上滑" : "下滑";
    }

    private void pickToolbarMode(boolean hidden) {
        OptionSheet sheet = new OptionSheet(requireContext(), "显示方式", null);
        sheet.option("输入时显示", !hidden, () -> saveToolbar(AndroidLocalSettings.TOOLBAR_HIDDEN, false));
        sheet.option("隐藏", hidden, () -> saveToolbar(AndroidLocalSettings.TOOLBAR_HIDDEN, true));
        sheet.show();
    }

    private void pickToolbarSize(boolean custom, int current) {
        OptionSheet sheet = new OptionSheet(requireContext(), "工具栏尺寸", null);
        sheet.option("跟随候选栏", !custom,
            () -> KeyboardSheets.saveLocal(this, AndroidLocalSettings.TOOLBAR_SIZE_PERCENT, null,
                this::reload, this::reload));
        for (int percent = ToolbarSizePolicy.MIN_PERCENT; percent <= ToolbarSizePolicy.MAX_PERCENT; percent += 10) {
            int choice = percent;
            sheet.option(percent + "%", custom && current == percent,
                () -> saveLocalSetting(AndroidLocalSettings.TOOLBAR_SIZE_PERCENT, choice));
        }
        sheet.show();
    }

    /** 回到 100% 时从文档里删掉这一项，与共享设置页 `candidateScalePatch` 写缺省值的方式相同。 */
    private void saveCandidateScale(int value) {
        int percent = CandidateScalePolicy.percent(value);
        savePreferences(preferences -> {
            if (percent == CandidateScalePolicy.DEFAULT_PERCENT) preferences.remove("candidate_scale_percent");
            else preferences.put("candidate_scale_percent", percent);
        });
    }

    private static String animationLabel(String value) {
        for (int index = 0; index < ANIMATIONS.length; index++) {
            if (ANIMATIONS[index].equals(value)) return ANIMATION_LABELS[index];
        }
        return ANIMATION_LABELS[ANIMATION_LABELS.length - 1];
    }

    private void savePreference(String key, Object value) {
        savePreferences(preferences -> preferences.put(key, value));
    }

    private void renderTrackpad(LinearLayout column, JSONObject preferences, JSONObject options) {
        GroupCard group = GroupCard.add(column, "文本编辑触控板");
        TextTrackpadSettings value = TextTrackpadSettings.from(preferences);
        JSONObject horizontal = options == null ? null : options.optJSONObject("horizontal_dp");
        JSONObject vertical = options == null ? null : options.optJSONObject("vertical_dp");
        if (value == null || horizontal == null || vertical == null) {
            group.note("触控板设置尚未就绪，请返回后重试。");
            return;
        }
        group.toggle("优先使用触控板", null,
            value.enabled(), checked -> saveTrackpad("enabled", checked)).help("打开文本编辑时显示触控板，可在面板中切回方向键");
        group.toggle("按住工具临时打开", null,
            value.holdToOpen(), checked -> saveTrackpad("hold_to_open", checked)).help("按住工具栏的文本编辑按钮打开触控板，松手返回之前的面板");
        group.slider("水平移动距离", null,
            NumberPolicy.strictInt(horizontal, "minimum", -1), NumberPolicy.strictInt(horizontal, "maximum", -1),
            1, value.horizontalDp(), number -> number + " dp", number -> saveTrackpadDistance("horizontal_dp", number)).help("距离越小，左右移动字符越快");
        group.slider("垂直移动距离", null,
            NumberPolicy.strictInt(vertical, "minimum", -1), NumberPolicy.strictInt(vertical, "maximum", -1),
            1, value.verticalDp(), number -> number + " dp", number -> saveTrackpadDistance("vertical_dp", number)).help("距离越小，上下移动行越快");
        group.toggle("双击选词与三击选行", null,
            value.tapSelection(), checked -> saveTrackpad("tap_selection", checked)).help("编辑器提供足够文字时生效；长按后拖动也可延伸选区");
        group.toggle("逐步振动", null, value.haptics(),
            checked -> saveTrackpad("haptics", checked)).help("跟随键盘振动和系统触感开关");
        group.toggle("显示移动轨迹", null, value.trail(), checked -> saveTrackpad("trail", checked));
        group.toggle("光标文字放大镜", null,
            value.magnifier(), checked -> saveTrackpad("magnifier", checked)).help("显示光标附近文字；密码使用掩码，编辑器不提供文字时显示提示");
    }

    private void saveTrackpad(String key, Object value) {
        savePreferences(preferences -> KeyboardSheets.child(preferences, "android_text_trackpad").put(key, value));
    }

    /** 两个距离滑块保存成功后不重建页面，只在失败时刷新把滑块拨回磁盘上的值：`MsSlider` 对方向键和读屏调整每一格都提交，成功也重建的话旧的 SeekBar 节点被移除，读屏用户每调一格就丢一次焦点（与「键盘底部高度」相同，#10513）。滑块旁的数值由滑块自己更新，其他行不依赖这两个值。 */
    private void saveTrackpadDistance(String key, int value) {
        KeyboardSheets.save(this, preferences -> KeyboardSheets.child(preferences, "android_text_trackpad").put(key, value),
            null, this::reload);
    }

    /** 改一个工具栏开关（`touch_toolbar` 的成员、语音的 `touch_voice_shortcut` 或本地设置），写完重画预览（显示方式还会改变按钮行的可用状态）。 */
    private void saveToolbar(String member, boolean value) {
        if (AndroidLocalSettings.specs().containsKey(member)) {
            KeyboardSheets.saveLocal(this, member, value, this::reload, this::reload);
            return;
        }
        if (member.equals(ToolbarOrderPolicy.VOICE_SHORTCUT_PREFERENCE)) {
            KeyboardSheets.save(this, preferences -> preferences.put(member, value), this::reload, this::reload);
            return;
        }
        KeyboardSheets.save(this, preferences -> KeyboardSheets.child(preferences, "touch_toolbar").put(member, value),
            this::reload, this::reload);
    }

    /** 本地的按键音 / 按键振动；传 null 的那一项保持不变。 */
    private void saveFeedback(@Nullable Boolean sound, @Nullable Boolean haptics) {
        HostTask.run(this, context -> {
            KeyboardFeedbackStore.Settings current = KeyboardFeedbackStore.load(context);
            KeyboardFeedbackPreferences.HapticStrength strength = current.hapticStrength();
            try {
                KeyboardFeedbackStore.save(context, new KeyboardFeedbackStore.Settings(
                    sound == null ? current.soundEnabled() : sound,
                    haptics == null ? current.hapticsEnabled() : haptics, strength));
                // 按键音和振动随「设置」同步；不标记的话这次改动传不上去，别的设备一改设置，下载时还会把它盖回去。
                SyncSignals.markDirty(context, SyncSwitch.SETTINGS);
                return Boolean.TRUE;
            } catch (java.io.IOException error) {
                return null;
            }
        }, saved -> {
            if (saved == null) {
                MsToast.show(requireContext(), "保存失败，请重试");
                reload();
            }
        });
    }
}
