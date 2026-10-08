package app.msime.android.home;

import app.msime.android.TextPolicy;

import android.content.Context;
import android.content.SharedPreferences;
import android.content.res.ColorStateList;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.graphics.Color;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.text.Editable;
import android.text.InputFilter;
import android.text.InputType;
import android.text.TextWatcher;
import android.view.View;
import android.view.ViewGroup;
import android.widget.EditText;
import android.widget.FrameLayout;
import android.widget.HorizontalScrollView;
import android.widget.LinearLayout;
import android.widget.ProgressBar;
import android.widget.TextView;
import androidx.annotation.NonNull;
import androidx.annotation.Nullable;
import androidx.lifecycle.ViewModel;
import androidx.lifecycle.ViewModelProvider;
import app.msime.android.CloudApi;
import app.msime.android.BitmapPolicy;
import app.msime.android.BoundsPolicy;
import app.msime.android.CustomKeyboardSkin;
import app.msime.android.CustomSkinLibrary;
import app.msime.android.KeyboardSkin;
import app.msime.android.PhotoDecodePolicy;
import app.msime.android.SkinJobsApi;
import app.msime.android.ViewPolicy;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Base64;
import java.util.List;
import java.util.UUID;
import java.util.concurrent.atomic.AtomicBoolean;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * AI 设计皮肤：上面是实时预览（26 键 / 9 键分段、配色圆点），下面是描述框和建议 chip、按键音效与按键动画两组分段，底部「✦ 生成皮肤」；生成后变成「✦ 重新生成」和「使用此皮肤」。
 *
 * <p>生成走 {@link SkinJobsApi}：client-core 拼提示词并校验模型给出的三套设计（`NativeClient.aiSkinPlan`），每套再由 `/v1/skins/jobs` 生成一张背景图；三套设计在预览上方用分段切换。生成期间预览变淡到 45 % 并显示「正在设计…」。生成状态和结果放在 {@link State} 这个 ViewModel 里：旋转、换深浅模式和 `recreate()` 重建页面时生成继续、结果保留；只有页面真的被关掉（返回或 activity 结束）时才取消，已经开出的任务都会被删除。服务端每天限额，429 时提示「今天的生成次数已用完」；503 说明这个部署没有开 AI 皮肤，页面和皮肤页的入口一起隐藏一天。
 *
 * <p>「使用此皮肤」把设计（背景图压成 JPEG 放进 `photo`，并带上所选的按键音效与动画，P23）存进自定义皮肤库、选中它、在统计里记一次 `record_skin`，提示「已使用「名」」后回到皮肤页。
 */
public final class AiSkinPage extends DetailPage {
    private static final String STORE = "msime_ai_skin";
    private static final String UNAVAILABLE_UNTIL = "unavailable_until";
    private static final long HIDE_MILLIS = 24L * 60 * 60 * 1000;
    /** CustomKeyboardSkin 接受的照片上限是 512000 字节，留一点余量。 */
    private static final int MAX_PHOTO_BYTES = 480_000;
    private static final int MAX_PHOTO_EDGE = 1080;

    private static final String[] SUGGESTIONS = {"秋天的银杏", "深夜霓虹", "宋代青瓷", "樱花与和纸", "雨后竹林", "复古终端"};
    private static final String[] SOUND_LABELS = {"木质", "清脆", "泡泡", "打字机", "静音"};
    private static final String[] SOUND_PACKS = {"msime-woodblock", CustomKeyboardSkin.DEFAULT_SOUND_PACK,
        "msime-bubble", "msime-typewriter", CustomKeyboardSkin.SILENT_SOUND_PACK};
    private static final String[] ANIMATION_LABELS = {"弹起", "涟漪", "发光", "浮起", "无"};
    private static final String[] ANIMATIONS = {"bounce", "ripple", "glow", "lift", CustomKeyboardSkin.DEFAULT_PRESS_ANIMATION};

    /** 一套生成好的设计：`design` 已经带上背景照片，按键音效和动画在使用时再按分段合进去。 */
    private record Result(String name, String description, JSONObject design) {}

    private static final String SAVED_PROMPT = "ai_skin_prompt";
    private static final String SAVED_NINE_KEY = "ai_skin_nine_key";
    private static final String SAVED_SOUND = "ai_skin_sound";
    private static final String SAVED_ANIMATION = "ai_skin_animation";

    /**
     * 页面的生成状态。放在 ViewModel 里而不是 Fragment 上：配置变化和 `recreate()` 会换一个新的 Fragment 实例，生成要几分钟，旧实例被销毁时不能把任务取消、把结果丢掉。
     *
     * <p>工作线程的结果先落到这里（{@link #complete}），页面有视图时再经 {@link #observer} 重画；没有视图时等下一次 {@link #buildContent} 读出来。描述、布局、音效和动画这几个小值另外存进 `onSaveInstanceState`，进程被杀后也能恢复；设计 JSON 只留在这里，不进 Bundle。
     */
    public static final class State extends ViewModel {
        private final Handler main = new Handler(Looper.getMainLooper());
        final List<Result> results = new ArrayList<>(SkinJobsApi.MAX_DESIGNS);
        int chosen;
        boolean nineKey;
        int sound = 1;
        int animation = 4;
        String prompt = "";
        String generatedFrom = "";
        boolean busy;
        /** 已经从 onSaveInstanceState 恢复过（或本来就是新页面），之后的 onCreate 不再覆盖。 */
        boolean initialized;
        /** 生成结束时要提示的一句话，等页面有视图时再显示。 */
        @Nullable String pendingMessage;
        @Nullable AtomicBoolean cancelled;
        /** 页面有视图时设置，结果到了就调用它重画；onDestroyView 时清掉。 */
        @Nullable Runnable observer;

        void start(Context application, String text) {
            prompt = text;
            busy = true;
            AtomicBoolean flag = new AtomicBoolean(false);
            cancelled = flag;
            // 一次生成可能要几分钟，不能占用设置页共用的那条 HostTask 线程。
            Thread worker = new Thread(() -> {
                List<Result> generated = new ArrayList<>(SkinJobsApi.MAX_DESIGNS);
                CloudApi.Failure failure = null;
                try {
                    for (SkinJobsApi.Proposal proposal : new SkinJobsApi(new CloudApi(application)).generate(text, flag)) {
                        generated.add(new Result(proposal.name(), proposal.description(), withPhoto(proposal)));
                    }
                } catch (CloudApi.Failure error) {
                    failure = error;
                } catch (RuntimeException error) {
                    android.util.Log.w("MSIMESettings", "AI skin generation failed", error);
                    failure = new CloudApi.Failure(0, "ai_skin_unavailable", String.valueOf(error), 0);
                }
                CloudApi.Failure result = failure;
                main.post(() -> complete(application, flag, text, generated, result));
            }, "msime-ai-skin-generate");
            worker.setDaemon(true);
            worker.start();
        }

        private void complete(Context application, AtomicBoolean flag, String text, List<Result> generated,
                @Nullable CloudApi.Failure failure) {
            if (cancelled != flag || flag.get()) return;
            busy = false;
            cancelled = null;
            if (failure != null) {
                if (SkinJobsApi.unavailable(failure)) {
                    store(application).edit()
                        .putLong(UNAVAILABLE_UNTIL, System.currentTimeMillis() + HIDE_MILLIS).apply();
                }
                if (!"cancelled".equals(failure.code)) pendingMessage = SkinJobsApi.message(failure);
            } else if (!generated.isEmpty()) {
                results.clear();
                results.addAll(generated);
                chosen = 0;
                generatedFrom = text;
                presetFeedback(CustomKeyboardSkin.from(generated.get(0).design()).keyMaterial());
            }
            Runnable notify = observer;
            if (notify != null) notify.run();
        }

        /** 生成结果预选一组音效与动画：按设计的键帽材质挑一组相配的，用户可以再改。 */
        private void presetFeedback(String material) {
            switch (material) {
                case "raised" -> { sound = 0; animation = 0; }
                case "glass" -> { sound = 2; animation = 2; }
                case "paper" -> { sound = 3; animation = 3; }
                default -> { sound = 1; animation = 1; }
            }
        }

        /** 页面真的被关掉时才走到这里；配置变化和 `recreate()` 不会。 */
        @Override protected void onCleared() {
            AtomicBoolean flag = cancelled;
            if (flag != null) flag.set(true);
            observer = null;
        }
    }

    @Nullable private State state;
    @Nullable private LinearLayout column;
    @Nullable private KeyboardSkin currentSkin;
    private boolean unavailable;
    /** 「使用此皮肤」正在保存；挡住连点存出两份。成功后页面就退出了，所以只在失败时清掉。 */
    private boolean saving;

    @Nullable private KeyboardPreview preview;
    @Nullable private TextView title;
    @Nullable private TextView subtitle;
    @Nullable private LinearLayout palette;
    @Nullable private View busyOverlay;

    /** 入口是否因为服务端 503 暂时隐藏。只读本应用自己的 SharedPreferences，可以在主线程调用。 */
    static boolean hidden(Context context) {
        return System.currentTimeMillis() < store(context).getLong(UNAVAILABLE_UNTIL, 0L);
    }

    private static SharedPreferences store(Context context) {
        return context.getApplicationContext().getSharedPreferences(STORE, Context.MODE_PRIVATE);
    }

    @Override public void onCreate(@Nullable Bundle saved) {
        super.onCreate(saved);
        State current = new ViewModelProvider(this).get(State.class);
        if (!current.initialized) {
            current.initialized = true;
            if (saved != null) {
                current.prompt = saved.getString(SAVED_PROMPT, "");
                current.nineKey = saved.getBoolean(SAVED_NINE_KEY, false);
                current.sound = clampIndex(saved.getInt(SAVED_SOUND, current.sound), SOUND_PACKS.length);
                current.animation = clampIndex(saved.getInt(SAVED_ANIMATION, current.animation), ANIMATIONS.length);
            }
        }
        state = current;
    }

    @Override public void onSaveInstanceState(@NonNull Bundle out) {
        super.onSaveInstanceState(out);
        State current = state;
        if (current == null) return;
        out.putString(SAVED_PROMPT, current.prompt);
        out.putBoolean(SAVED_NINE_KEY, current.nineKey);
        out.putInt(SAVED_SOUND, current.sound);
        out.putInt(SAVED_ANIMATION, current.animation);
    }

    private static int clampIndex(int value, int size) {
        return value < 0 || value >= size ? 0 : value;
    }

    @Override protected void buildContent(LinearLayout column, Bundle args) {
        this.column = column;
        unavailable = hidden(requireContext());
        State current = state();
        current.observer = this::onStateChanged;
        render();
        showPendingMessage();
        boolean dark = AppMode.dark(requireContext());
        HostTask.run(this, context -> {
            JSONObject preferences = KeyboardSheets.preferences(context);
            return preferences == null ? KeyboardSkin.system(dark, HostStore.seed(context))
                : HostStore.keyboardSkin(preferences, dark, HostStore.seed(context));
        }, skin -> {
            currentSkin = skin;
            refreshPreview();
        });
    }

    /** 生成结束：页面有视图时才会被调用（observer 在 onDestroyView 时清掉）。 */
    private void onStateChanged() {
        if (!isAdded() || column == null) return;
        unavailable = hidden(requireContext());
        render();
        showPendingMessage();
    }

    private void showPendingMessage() {
        State current = state();
        String message = current.pendingMessage;
        if (message == null) return;
        current.pendingMessage = null;
        // 只在页面有视图时显示，画在当前 Activity 上；页面不在眼前时消息留到回来再说。
        MsToast.show(requireContext(), message);
    }

    private State state() {
        State current = state;
        if (current == null) throw new IllegalStateException("AiSkinPage used before onCreate");
        return current;
    }

    @Override public void onDestroyView() {
        State current = state;
        if (current != null) current.observer = null;
        column = null;
        preview = null;
        title = null;
        subtitle = null;
        palette = null;
        busyOverlay = null;
        super.onDestroyView();
    }

    private void render() {
        LinearLayout target = column;
        if (target == null) return;
        target.removeAllViews();
        Context context = requireContext();
        State s = state();

        GroupCard previewGroup = GroupCard.add(target, null);
        LinearLayout card = previewGroup.card();
        Ui.setPaddingDp(card, context, 14, 14, 14, 12);
        LinearLayout header = Ui.row(context);
        ViewPolicy.setCenteredVertically(header);
        LinearLayout heading = Ui.column(context);
        title = Ui.styledLabel(context, "", 17, 600, Ui.text(context));
        ViewPolicy.setSingleLine(title);
        heading.addView(title);
        subtitle = Ui.styledLabel(context, "", 13, 400, Ui.subText(context));
        heading.addView(subtitle);
        header.addView(heading, Ui.weightWrap(1f));
        SegmentedControl layout = new SegmentedControl(context);
        layout.setOptions(List.of("26 键", "9 键"), s.nineKey ? 1 : 0);
        layout.setOnSelect(index -> {
            s.nineKey = index == 1;
            refreshPreview();
        });
        header.addView(layout, Ui.wrap());
        card.addView(header);

        FrameLayout stage = new FrameLayout(context);
        preview = new KeyboardPreview(context);
        preview.setContentDescription("皮肤预览");
        stage.addView(preview, Ui.frameMatchWidthHeight(context, 200));
        LinearLayout overlay = Ui.column(context);
        ViewPolicy.setCentered(overlay);
        ProgressBar spinner = new ProgressBar(context);
        spinner.setIndeterminateTintList(ColorStateList.valueOf(Ui.accent(context)));
        overlay.addView(spinner, Ui.squareParams(context, 32));
        TextView designing = Ui.styledLabel(context, "正在设计…", 14, 500, Ui.text(context));
        overlay.addView(designing);
        busyOverlay = overlay;
        stage.addView(overlay, new FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT,
            ViewGroup.LayoutParams.MATCH_PARENT));
        LinearLayout.LayoutParams stageParams = Ui.matchWidth();
        stageParams.topMargin = Ui.dp(context, 12);
        card.addView(stage, stageParams);

        LinearLayout colours = Ui.row(context);
        ViewPolicy.setCenteredVertically(colours);
        TextView label = Ui.styledLabel(context, "配色", 13, 400, Ui.subText(context));
        colours.addView(label);
        palette = Ui.row(context);
        LinearLayout.LayoutParams paletteParams = Ui.wrap();
        paletteParams.setMarginStart(Ui.dp(context, 10));
        colours.addView(palette, paletteParams);
        LinearLayout.LayoutParams coloursParams = Ui.matchWidth();
        coloursParams.topMargin = Ui.dp(context, 10);
        card.addView(colours, coloursParams);

        if (s.results.size() > 1) {
            GroupCard choices = GroupCard.add(target, "方案");
            ViewPolicy.clearBackground(choices.card());
            SegmentedControl picker = new SegmentedControl(context);
            picker.setFillWidth(true);
            List<String> names = new ArrayList<>(s.results.size());
            for (Result result : s.results) names.add(result.name());
            picker.setOptions(names, s.chosen);
            picker.setOnSelect(index -> {
                s.chosen = index;
                refreshPreview();
            });
            choices.card().addView(picker, Ui.matchWidth());
        }

        GroupCard describe = GroupCard.add(target, "描述");
        EditText input = Ui.styledInput(context, Ui.TEXT_ROW_TITLE, 400, Ui.text(context));
        input.setText(s.prompt);
        input.setHint("写下你想要的样子，例如「雨后竹林」");
        input.setHintTextColor(Ui.subText(context));
        input.setInputType(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_FLAG_MULTI_LINE);
        ViewPolicy.setMinLines(input, 2);
        ViewPolicy.setTopStart(input);
        input.setFilters(new InputFilter[] {new InputFilter.LengthFilter(SkinJobsApi.MAX_PROMPT_CHARACTERS)});
        ViewPolicy.clearBackground(input);
        Ui.setPaddingDp(input, context, 16, 12, 16, 4);
        ViewPolicy.setEnabled(input, !s.busy);
        describe.card().addView(input, Ui.matchWidth());
        HorizontalScrollView chipScroll = new HorizontalScrollView(context);
        chipScroll.setHorizontalScrollBarEnabled(false);
        LinearLayout chips = Ui.row(context);
        Ui.setPaddingDp(chips, context, 12, 4, 12, 12);
        List<TextView> chipViews = new ArrayList<>(SUGGESTIONS.length);
        for (String suggestion : SUGGESTIONS) {
            TextView chip = Ui.styledLabel(context, suggestion, 13, 400, Ui.text(context));
            ViewPolicy.setSingleLine(chip);
            Ui.setSymmetricPaddingDp(chip, context, 12, 6);
            ViewPolicy.setInteractive(chip, true);
            ViewPolicy.bindClick(chip, () -> {
                if (s.busy) return;
                input.setText(suggestion);
                input.setSelection(input.length());
            });
            chip.setAccessibilityDelegate(KeyboardSheets.buttonDelegate("建议描述 " + suggestion));
            LinearLayout.LayoutParams chipParams = Ui.wrap();
            chipParams.setMarginEnd(Ui.dp(context, 8));
            chips.addView(chip, chipParams);
            chipViews.add(chip);
        }
        chipScroll.addView(chips);
        describe.card().addView(chipScroll, Ui.matchWidth());
        styleChips(context, chipViews);

        GroupCard soundGroup = GroupCard.add(target, "按键音效");
        ViewPolicy.clearBackground(soundGroup.card());
        SegmentedControl sounds = new SegmentedControl(context);
        sounds.setFillWidth(true);
        sounds.setOptions(List.of(SOUND_LABELS), s.sound);
        sounds.setOnSelect(index -> {
            s.sound = index;
            refreshPreview();
        });
        soundGroup.card().addView(sounds, Ui.matchWidth());

        GroupCard animationGroup = GroupCard.add(target, "按键动画");
        ViewPolicy.clearBackground(animationGroup.card());
        SegmentedControl animations = new SegmentedControl(context);
        animations.setFillWidth(true);
        animations.setOptions(List.of(ANIMATION_LABELS), s.animation);
        animations.setOnSelect(index -> s.animation = index);
        animationGroup.card().addView(animations, Ui.matchWidth());

        LinearLayout actions = Ui.row(context);
        LinearLayout.LayoutParams actionsParams = Ui.matchWidth();
        actionsParams.topMargin = Ui.dp(context, Ui.GROUP_GAP);
        if (unavailable) {
            GroupCard.add(target, null).note("AI 设计皮肤暂不可用，请稍后再来。");
        } else if (s.results.isEmpty()) {
            TextView generate = KeyboardSheets.bigButton(context, "✦ 生成皮肤", true, () -> generate(input));
            actions.addView(generate, Ui.weightWrap(1f));
            target.addView(actions, actionsParams);
            bindEnabled(input, generate);
        } else {
            TextView again = KeyboardSheets.bigButton(context, "✦ 重新生成", false, () -> generate(input));
            actions.addView(again, Ui.weightWrap(1f));
            TextView use = KeyboardSheets.bigButton(context, "使用此皮肤", true, this::useResult);
            LinearLayout.LayoutParams useParams = Ui.weightWrap(1f);
            useParams.setMarginStart(Ui.dp(context, 12));
            actions.addView(use, useParams);
            target.addView(actions, actionsParams);
            bindEnabled(input, again);
            ViewPolicy.setEnabledWithAlpha(use, !s.busy && !saving, 0.38f);
        }
        input.addTextChangedListener(new TextWatcher() {
            @Override public void beforeTextChanged(CharSequence text, int start, int count, int after) {}

            @Override public void onTextChanged(CharSequence text, int start, int before, int count) {}

            @Override public void afterTextChanged(Editable text) {
                s.prompt = text.toString();
                styleChips(context, chipViews);
            }
        });
        refreshPreview();
    }

    /** 描述为空或正在生成时按钮不可用。 */
    private void bindEnabled(EditText input, TextView button) {
        State s = state();
        Runnable update = () -> {
            boolean enabled = !s.busy && !TextPolicy.trimmed(input.getText().toString()).isEmpty();
            ViewPolicy.setEnabledWithAlpha(button, enabled, 0.38f);
        };
        update.run();
        input.addTextChangedListener(new TextWatcher() {
            @Override public void beforeTextChanged(CharSequence text, int start, int count, int after) {}

            @Override public void onTextChanged(CharSequence text, int start, int before, int count) {}

            @Override public void afterTextChanged(Editable text) { update.run(); }
        });
    }

    /** 和描述相同的 chip 填强调色，其余是 accentSoft 底。 */
    private void styleChips(Context context, List<TextView> chips) {
        State s = state();
        String current = TextPolicy.trimmed(s.prompt);
        for (TextView chip : chips) {
            boolean on = chip.getText().toString().equals(current);
            Ui.style(chip, 13, on ? 600 : 400, on ? Ui.onAccent(context) : Ui.text(context));
            ViewPolicy.setBackground(chip, Ui.pillRipple(context, on ? Ui.accent(context) : Ui.rowBackground(context)));
        }
    }

    private void refreshPreview() {
        KeyboardPreview view = preview;
        if (view == null) return;
        Context context = requireContext();
        State s = state();
        Result result = s.results.isEmpty() ? null
            : s.results.get(BoundsPolicy.atMost(s.chosen, s.results.size() - 1));
        KeyboardSkin skin = result == null ? currentSkin
            : KeyboardSkin.custom(result.design(), AppMode.dark(context));
        view.setKeyboard(skin, s.nineKey);
        ViewPolicy.setActiveAlpha(view, !s.busy, 0.45f);
        if (busyOverlay != null) ViewPolicy.setVisible(busyOverlay, s.busy);
        if (title != null) title.setText(result == null ? "未命名皮肤" : result.name());
        if (subtitle != null) {
            subtitle.setText(s.busy ? "正在根据描述生成…"
                : result == null ? "写下描述，AI 生成配色、音效和动画" : "根据「" + s.generatedFrom + "」生成");
        }
        LinearLayout dots = palette;
        if (dots != null) {
            dots.removeAllViews();
            if (skin != null) {
                String[] colours = {skin.background(), skin.keyBackground(), skin.functionBackground(),
                    skin.keyForeground(), skin.returnBackground()};
                for (String colour : colours) {
                    View dot = new View(context);
                    android.graphics.drawable.GradientDrawable shape = Ui.outlined(
                        Ui.parseColor(colour, Color.GRAY), 9999f,
                        Ui.atLeastOnePx(context, 1), Ui.hairline(context));
                    ViewPolicy.setBackground(dot, shape);
                    LinearLayout.LayoutParams params = Ui.squareParams(context, 16);
                    params.setMarginEnd(Ui.dp(context, 6));
                    dots.addView(dot, params);
                }
            }
        }
    }

    private void generate(EditText input) {
        State s = state();
        String text = TextPolicy.trimmed(input.getText().toString());
        if (s.busy || text.isEmpty()) return;
        s.start(requireContext().getApplicationContext(), text);
        render();
    }

    /** 把背景图压成不超过 {@link #MAX_PHOTO_BYTES} 的 JPEG 放进设计的 `photo`；压不下去时不带照片。 */
    private static JSONObject withPhoto(SkinJobsApi.Proposal proposal) {
        JSONObject design;
        try {
            design = new JSONObject(proposal.design().toString());
        } catch (JSONException error) {
            design = new JSONObject();
        }
        byte[] bytes;
        try {
            bytes = Base64.getDecoder().decode(proposal.artwork().base64());
        } catch (IllegalArgumentException error) {
            return CustomKeyboardSkin.from(design).toJson(true);
        }
        BitmapFactory.Options bounds = new BitmapFactory.Options();
        bounds.inJustDecodeBounds = true;
        BitmapFactory.decodeByteArray(bytes, 0, bytes.length, bounds);
        int sample = PhotoDecodePolicy.sampleSize(bounds.outWidth, bounds.outHeight);
        if (sample == 0) return CustomKeyboardSkin.from(design).toJson(true);
        BitmapFactory.Options options = new BitmapFactory.Options();
        options.inSampleSize = sample;
        Bitmap bitmap = BitmapFactory.decodeByteArray(bytes, 0, bytes.length, options);
        if (bitmap == null || !PhotoDecodePolicy.withinBounds(bitmap.getWidth(), bitmap.getHeight())) {
            if (bitmap != null) bitmap.recycle();
            return CustomKeyboardSkin.from(design).toJson(true);
        }
        bitmap = BitmapPolicy.scaleToEdge(bitmap, MAX_PHOTO_EDGE);
        byte[] jpeg = BitmapPolicy.compressJpegUnderBytes(bitmap, MAX_PHOTO_BYTES);
        bitmap.recycle();
        try {
            if (jpeg != null) design.put("photo", Base64.getEncoder().encodeToString(jpeg));
        } catch (JSONException error) {
            // 键是常量、值是字符串，org.json 不会在这里抛出；真抛了就不带照片。
            android.util.Log.w("MSIMESettings", "AI skin photo dropped", error);
        }
        return CustomKeyboardSkin.from(design).toJson(true);
    }

    private void useResult() {
        State s = state();
        if (s.busy || saving || s.results.isEmpty()) return;
        Result result = s.results.get(BoundsPolicy.atMost(s.chosen, s.results.size() - 1));
        JSONObject design = CustomKeyboardSkin.from(result.design())
            .withFeedback(SOUND_PACKS[s.sound], ANIMATIONS[s.animation]).toJson(true);
        String name = result.name();
        saving = true;
        HostTask.run(this, context -> save(context, name, design), failure -> {
            if (failure == null) {
                saving = false;
                return;
            }
            if (!failure.isEmpty()) {
                saving = false;
                MsToast.show(requireContext(), failure);
                return;
            }
            MsToast.show(requireContext(), "已使用「" + name + "」");
            requireActivity().getOnBackPressedDispatcher().onBackPressed();
        });
    }

    /** 存进皮肤库并选中；成功返回空字符串，否则是要提示的原因。 */
    private static String save(Context context, String name, JSONObject design) {
        String directory = HostStore.directory(context);
        if (directory.isEmpty()) return "请先完成首次设置";
        String id = UUID.randomUUID().toString();
        try {
            if (!CustomSkinLibrary.add(Paths.get(directory), id, name, design))
                return "皮肤库已满（最多 12 个），请先在键盘的皮肤面板里删掉一些";
        } catch (java.io.IOException error) {
            return "保存皮肤失败，请重试";
        }
        JSONObject saved = KeyboardSheets.write(context, preferences -> KeyboardSheets.applyDesign(preferences, design));
        if (saved == null) return "皮肤已保存到我的设计，但切换失败，请在皮肤页里选它";
        KeyboardSheets.applyLocalFeedback(context, design);
        KeyboardSheets.recordSkin(context, id);
        return "";
    }

}
