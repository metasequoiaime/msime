package app.msime.android.home;

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
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.widget.EditText;
import android.widget.FrameLayout;
import android.widget.HorizontalScrollView;
import android.widget.LinearLayout;
import android.widget.ProgressBar;
import android.widget.TextView;
import androidx.annotation.Nullable;
import app.msime.android.CloudApi;
import app.msime.android.CustomKeyboardSkin;
import app.msime.android.CustomSkinLibrary;
import app.msime.android.KeyboardSkin;
import app.msime.android.SkinJobsApi;
import java.io.ByteArrayOutputStream;
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
 * <p>生成走 {@link SkinJobsApi}：client-core 拼提示词并校验模型给出的三套设计（`NativeClient.aiSkinPlan`），每套再由 `/v1/skins/jobs` 生成一张背景图；三套设计在预览上方用分段切换。生成期间预览变淡到 45 % 并显示「正在设计…」，离开页面即取消，已经开出的任务都会被删除。服务端每天限额，429 时提示「今天的生成次数已用完」；503 说明这个部署没有开 AI 皮肤，页面和皮肤页的入口一起隐藏一天。
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

    private final Handler main = new Handler(Looper.getMainLooper());
    @Nullable private LinearLayout column;
    @Nullable private KeyboardSkin currentSkin;
    private final List<Result> results = new ArrayList<>();
    private int chosen;
    private boolean nineKey;
    private int sound = 1;
    private int animation = 4;
    private String prompt = "";
    private String generatedFrom = "";
    private boolean busy;
    private boolean unavailable;
    @Nullable private AtomicBoolean cancelled;

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

    @Override protected void buildContent(LinearLayout column, Bundle args) {
        this.column = column;
        unavailable = hidden(requireContext());
        render();
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

    @Override public void onDestroyView() {
        column = null;
        preview = null;
        title = null;
        subtitle = null;
        palette = null;
        busyOverlay = null;
        super.onDestroyView();
    }

    @Override public void onDestroy() {
        AtomicBoolean flag = cancelled;
        if (flag != null) flag.set(true);
        super.onDestroy();
    }

    private void render() {
        LinearLayout target = column;
        if (target == null) return;
        target.removeAllViews();
        Context context = requireContext();

        GroupCard previewGroup = GroupCard.add(target, null);
        LinearLayout card = previewGroup.card();
        card.setPadding(Ui.dp(context, 14), Ui.dp(context, 14), Ui.dp(context, 14), Ui.dp(context, 12));
        LinearLayout header = new LinearLayout(context);
        header.setOrientation(LinearLayout.HORIZONTAL);
        header.setGravity(Gravity.CENTER_VERTICAL);
        LinearLayout heading = new LinearLayout(context);
        heading.setOrientation(LinearLayout.VERTICAL);
        title = new TextView(context);
        title.setSingleLine(true);
        Ui.style(title, 17, 600, Ui.text(context));
        heading.addView(title);
        subtitle = new TextView(context);
        Ui.style(subtitle, 13, 400, Ui.subText(context));
        heading.addView(subtitle);
        header.addView(heading, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));
        SegmentedControl layout = new SegmentedControl(context);
        layout.setOptions(List.of("26 键", "9 键"), nineKey ? 1 : 0);
        layout.setOnSelect(index -> {
            nineKey = index == 1;
            refreshPreview();
        });
        header.addView(layout, KeyboardSheets.wrap());
        card.addView(header);

        FrameLayout stage = new FrameLayout(context);
        preview = new KeyboardPreview(context);
        preview.setContentDescription("皮肤预览");
        stage.addView(preview, new FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, Ui.dp(context, 200)));
        LinearLayout overlay = new LinearLayout(context);
        overlay.setOrientation(LinearLayout.VERTICAL);
        overlay.setGravity(Gravity.CENTER);
        ProgressBar spinner = new ProgressBar(context);
        spinner.setIndeterminateTintList(ColorStateList.valueOf(Ui.accent(context)));
        overlay.addView(spinner, new LinearLayout.LayoutParams(Ui.dp(context, 32), Ui.dp(context, 32)));
        TextView designing = new TextView(context);
        designing.setText("正在设计…");
        Ui.style(designing, 14, 500, Ui.text(context));
        overlay.addView(designing);
        busyOverlay = overlay;
        stage.addView(overlay, new FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT,
            ViewGroup.LayoutParams.MATCH_PARENT));
        LinearLayout.LayoutParams stageParams = new LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        stageParams.topMargin = Ui.dp(context, 12);
        card.addView(stage, stageParams);

        LinearLayout colours = new LinearLayout(context);
        colours.setOrientation(LinearLayout.HORIZONTAL);
        colours.setGravity(Gravity.CENTER_VERTICAL);
        TextView label = new TextView(context);
        label.setText("配色");
        Ui.style(label, 13, 400, Ui.subText(context));
        colours.addView(label);
        palette = new LinearLayout(context);
        palette.setOrientation(LinearLayout.HORIZONTAL);
        LinearLayout.LayoutParams paletteParams = KeyboardSheets.wrap();
        paletteParams.setMarginStart(Ui.dp(context, 10));
        colours.addView(palette, paletteParams);
        LinearLayout.LayoutParams coloursParams = new LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        coloursParams.topMargin = Ui.dp(context, 10);
        card.addView(colours, coloursParams);

        if (results.size() > 1) {
            GroupCard choices = GroupCard.add(target, "方案");
            choices.card().setBackground(null);
            SegmentedControl picker = new SegmentedControl(context);
            picker.setFillWidth(true);
            List<String> names = new ArrayList<>();
            for (Result result : results) names.add(result.name());
            picker.setOptions(names, chosen);
            picker.setOnSelect(index -> {
                chosen = index;
                refreshPreview();
            });
            choices.card().addView(picker, matchWidth());
        }

        GroupCard describe = GroupCard.add(target, "描述");
        EditText input = new EditText(context);
        input.setText(prompt);
        input.setHint("写下你想要的样子，例如「雨后竹林」");
        input.setHintTextColor(Ui.subText(context));
        input.setInputType(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_FLAG_MULTI_LINE);
        input.setMinLines(2);
        input.setGravity(Gravity.TOP | Gravity.START);
        input.setFilters(new InputFilter[] {new InputFilter.LengthFilter(SkinJobsApi.MAX_PROMPT_CHARACTERS)});
        input.setBackground(null);
        Ui.style(input, Ui.TEXT_ROW_TITLE, 400, Ui.text(context));
        input.setPadding(Ui.dp(context, 16), Ui.dp(context, 12), Ui.dp(context, 16), Ui.dp(context, 4));
        input.setEnabled(!busy);
        describe.card().addView(input, matchWidth());
        HorizontalScrollView chipScroll = new HorizontalScrollView(context);
        chipScroll.setHorizontalScrollBarEnabled(false);
        LinearLayout chips = new LinearLayout(context);
        chips.setOrientation(LinearLayout.HORIZONTAL);
        chips.setPadding(Ui.dp(context, 12), Ui.dp(context, 4), Ui.dp(context, 12), Ui.dp(context, 12));
        List<TextView> chipViews = new ArrayList<>();
        for (String suggestion : SUGGESTIONS) {
            TextView chip = new TextView(context);
            chip.setText(suggestion);
            chip.setSingleLine(true);
            chip.setPadding(Ui.dp(context, 12), Ui.dp(context, 6), Ui.dp(context, 12), Ui.dp(context, 6));
            chip.setClickable(true);
            chip.setFocusable(true);
            chip.setOnClickListener(ignored -> {
                if (busy) return;
                input.setText(suggestion);
                input.setSelection(input.length());
            });
            chip.setAccessibilityDelegate(KeyboardSheets.buttonDelegate("建议描述 " + suggestion));
            LinearLayout.LayoutParams chipParams = KeyboardSheets.wrap();
            chipParams.setMarginEnd(Ui.dp(context, 8));
            chips.addView(chip, chipParams);
            chipViews.add(chip);
        }
        chipScroll.addView(chips);
        describe.card().addView(chipScroll, matchWidth());
        styleChips(context, chipViews);

        GroupCard soundGroup = GroupCard.add(target, "按键音效");
        soundGroup.card().setBackground(null);
        SegmentedControl sounds = new SegmentedControl(context);
        sounds.setFillWidth(true);
        sounds.setOptions(List.of(SOUND_LABELS), sound);
        sounds.setOnSelect(index -> {
            sound = index;
            refreshPreview();
        });
        soundGroup.card().addView(sounds, matchWidth());

        GroupCard animationGroup = GroupCard.add(target, "按键动画");
        animationGroup.card().setBackground(null);
        SegmentedControl animations = new SegmentedControl(context);
        animations.setFillWidth(true);
        animations.setOptions(List.of(ANIMATION_LABELS), animation);
        animations.setOnSelect(index -> animation = index);
        animationGroup.card().addView(animations, matchWidth());

        LinearLayout actions = new LinearLayout(context);
        actions.setOrientation(LinearLayout.HORIZONTAL);
        LinearLayout.LayoutParams actionsParams = new LinearLayout.LayoutParams(
            ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        actionsParams.topMargin = Ui.dp(context, Ui.GROUP_GAP);
        if (unavailable) {
            GroupCard.add(target, null).note("AI 设计皮肤暂不可用，请稍后再来。");
        } else if (results.isEmpty()) {
            TextView generate = KeyboardSheets.bigButton(context, "✦ 生成皮肤", true, () -> generate(input));
            actions.addView(generate, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));
            target.addView(actions, actionsParams);
            bindEnabled(input, generate);
        } else {
            TextView again = KeyboardSheets.bigButton(context, "✦ 重新生成", false, () -> generate(input));
            actions.addView(again, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));
            TextView use = KeyboardSheets.bigButton(context, "使用此皮肤", true, this::useResult);
            LinearLayout.LayoutParams useParams = new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f);
            useParams.setMarginStart(Ui.dp(context, 12));
            actions.addView(use, useParams);
            target.addView(actions, actionsParams);
            bindEnabled(input, again);
            Ui.setEnabledLook(use, !busy);
            use.setEnabled(!busy);
        }
        input.addTextChangedListener(new TextWatcher() {
            @Override public void beforeTextChanged(CharSequence text, int start, int count, int after) {}

            @Override public void onTextChanged(CharSequence text, int start, int before, int count) {}

            @Override public void afterTextChanged(Editable text) {
                prompt = text.toString();
                styleChips(context, chipViews);
            }
        });
        refreshPreview();
    }

    /** 描述为空或正在生成时按钮不可用。 */
    private void bindEnabled(EditText input, TextView button) {
        Runnable update = () -> {
            boolean enabled = !busy && !input.getText().toString().trim().isEmpty();
            button.setEnabled(enabled);
            Ui.setEnabledLook(button, enabled);
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
        String current = prompt.trim();
        for (TextView chip : chips) {
            boolean on = chip.getText().toString().equals(current);
            Ui.style(chip, 13, on ? 600 : 400, on ? Ui.onAccent(context) : Ui.text(context));
            chip.setBackground(Ui.rippleOn(context, on ? Ui.accent(context) : Ui.rowBackground(context), 9999f));
        }
    }

    private void refreshPreview() {
        KeyboardPreview view = preview;
        if (view == null) return;
        Context context = requireContext();
        Result result = results.isEmpty() ? null : results.get(Math.min(chosen, results.size() - 1));
        KeyboardSkin skin = result == null ? currentSkin
            : KeyboardSkin.custom(result.design(), AppMode.dark(context));
        view.setKeyboard(skin, nineKey);
        view.setAlpha(busy ? 0.45f : 1f);
        if (busyOverlay != null) busyOverlay.setVisibility(busy ? View.VISIBLE : View.GONE);
        if (title != null) title.setText(result == null ? "未命名皮肤" : result.name());
        if (subtitle != null) {
            subtitle.setText(busy ? "正在根据描述生成…"
                : result == null ? "写下描述，AI 生成配色、音效和动画" : "根据「" + generatedFrom + "」生成");
        }
        LinearLayout dots = palette;
        if (dots != null) {
            dots.removeAllViews();
            if (skin != null) {
                String[] colours = {skin.background(), skin.keyBackground(), skin.functionBackground(),
                    skin.keyForeground(), skin.returnBackground()};
                for (String colour : colours) {
                    View dot = new View(context);
                    android.graphics.drawable.GradientDrawable shape = Ui.pill(parse(colour));
                    shape.setStroke(Math.max(1, Ui.dp(context, 1)), Ui.hairline(context));
                    dot.setBackground(shape);
                    LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(Ui.dp(context, 16), Ui.dp(context, 16));
                    params.setMarginEnd(Ui.dp(context, 6));
                    dots.addView(dot, params);
                }
            }
        }
    }

    private void generate(EditText input) {
        String text = input.getText().toString().trim();
        if (busy || text.isEmpty()) return;
        prompt = text;
        busy = true;
        AtomicBoolean flag = new AtomicBoolean(false);
        cancelled = flag;
        render();
        Context application = requireContext().getApplicationContext();
        // 一次生成可能要几分钟，不能占用设置页共用的那条 HostTask 线程。
        Thread worker = new Thread(() -> {
            List<Result> generated = new ArrayList<>();
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
            main.post(() -> finishGenerate(flag, text, generated, result));
        }, "msime-ai-skin-generate");
        worker.setDaemon(true);
        worker.start();
    }

    private void finishGenerate(AtomicBoolean flag, String text, List<Result> generated,
            @Nullable CloudApi.Failure failure) {
        if (cancelled != flag || !isAdded()) return;
        busy = false;
        cancelled = null;
        if (failure != null) {
            if (SkinJobsApi.unavailable(failure)) {
                store(requireContext()).edit()
                    .putLong(UNAVAILABLE_UNTIL, System.currentTimeMillis() + HIDE_MILLIS).apply();
                unavailable = true;
            }
            if (!"cancelled".equals(failure.code)) MsToast.show(requireContext(), SkinJobsApi.message(failure));
        } else if (!generated.isEmpty()) {
            results.clear();
            results.addAll(generated);
            chosen = 0;
            generatedFrom = text;
            presetFeedback(CustomKeyboardSkin.from(generated.get(0).design()).keyMaterial());
        }
        if (column != null) render();
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
        Bitmap bitmap = BitmapFactory.decodeByteArray(bytes, 0, bytes.length);
        if (bitmap == null) return CustomKeyboardSkin.from(design).toJson(true);
        int edge = Math.max(bitmap.getWidth(), bitmap.getHeight());
        if (edge > MAX_PHOTO_EDGE) {
            float scale = MAX_PHOTO_EDGE / (float) edge;
            Bitmap scaled = Bitmap.createScaledBitmap(bitmap, Math.max(1, Math.round(bitmap.getWidth() * scale)),
                Math.max(1, Math.round(bitmap.getHeight() * scale)), true);
            if (scaled != bitmap) bitmap.recycle();
            bitmap = scaled;
        }
        byte[] jpeg = null;
        for (int quality = 85; quality >= 40; quality -= 15) {
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            bitmap.compress(Bitmap.CompressFormat.JPEG, quality, out);
            if (out.size() <= MAX_PHOTO_BYTES) {
                jpeg = out.toByteArray();
                break;
            }
        }
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
        if (busy || results.isEmpty()) return;
        Result result = results.get(Math.min(chosen, results.size() - 1));
        JSONObject design = CustomKeyboardSkin.from(result.design())
            .withFeedback(SOUND_PACKS[sound], ANIMATIONS[animation]).toJson(true);
        String name = result.name();
        HostTask.run(this, context -> save(context, name, design), failure -> {
            if (failure == null) return;
            if (!failure.isEmpty()) {
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

    private static int parse(String colour) {
        try {
            return Color.parseColor(colour);
        } catch (IllegalArgumentException | NullPointerException error) {
            return Color.GRAY;
        }
    }

    private static LinearLayout.LayoutParams matchWidth() {
        return new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT);
    }
}
