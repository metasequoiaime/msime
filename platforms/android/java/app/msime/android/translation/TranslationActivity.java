package app.msime.android;

import android.app.Activity;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.os.ResultReceiver;
import android.text.Editable;
import android.text.TextWatcher;
import android.text.InputType;
import android.view.View;
import android.widget.AdapterView;
import android.widget.ArrayAdapter;
import android.widget.Button;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.Spinner;
import android.widget.TextView;
import app.msime.android.policy.HostOptionsPolicy;
import androidx.core.graphics.Insets;
import androidx.core.view.ViewCompat;
import androidx.core.view.WindowCompat;
import androidx.core.view.WindowInsetsCompat;
import static app.msime.android.ImeTranslationEntry.EDITOR;
import static app.msime.android.ImeTranslationEntry.RECEIVER;
import static app.msime.android.ImeTranslationEntry.TEXT;
import java.util.List;
import java.util.UUID;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import org.json.JSONObject;

/** 独立原文编辑连接，手写和键盘输入不会写进调用方聊天框。 */
@SuppressWarnings("deprecation")
public final class TranslationActivity extends Activity {
    private static final String[] CODES = {"zh", "en", "ja", "ko", "fr", "de", "es", "ru"};
    private static final String[] LABELS = {"中文", "英语", "日语", "韩语", "法语", "德语", "西班牙语", "俄语"};
    private final ExecutorService worker = Executors.newSingleThreadExecutor();
    private final Handler main = new Handler(Looper.getMainLooper());
    private final TranslationDraft draft = new TranslationDraft();
    private EditText source;
    private TextView result;
    private TextView status;
    private Button insert;
    private Button copy;
    private Button translate;
    private Spinner from;
    private Spinner to;
    private boolean busy;
    private boolean destroyed;
    private long voiceGeneration;

    @Override protected void onCreate(Bundle state) {
        super.onCreate(state);
        ScreenCaptureProtection.apply(this);
        WindowCompat.setDecorFitsSystemWindows(getWindow(), false);
        var systemBars = WindowCompat.getInsetsController(getWindow(), getWindow().getDecorView());
        systemBars.setAppearanceLightStatusBars(true);
        systemBars.setAppearanceLightNavigationBars(true);
        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        int pad = Math.round(16 * getResources().getDisplayMetrics().density);
        root.setPadding(pad, pad, pad, pad);
        ViewCompat.setOnApplyWindowInsetsListener(root, (view, insets) -> {
            Insets bars = insets.getInsets(WindowInsetsCompat.Type.systemBars());
            Insets ime = insets.getInsets(WindowInsetsCompat.Type.ime());
            root.setPadding(pad + bars.left, pad + bars.top, pad + bars.right,
                WindowInsetsPolicy.bottomContentInset(bars.bottom, 0, ime.bottom, pad));
            return insets;
        });
        root.setBackgroundColor(0xfffafafa);
        LinearLayout header = new LinearLayout(this);
        TextView title = label("翻译", 22);
        header.addView(title, new LinearLayout.LayoutParams(0, -2, 1));
        button(header, "返回", this::finish);
        button(header, "?", () -> ScreenCaptureProtection.show(new android.app.AlertDialog.Builder(this).setTitle("翻译")
            .setMessage("原文可用键盘、手写或语音输入。点击翻译后发送给当前翻译服务，预览后再插入。每次最多 256 字。")
            .setPositiveButton("知道了", null).create()));
        root.addView(header);
        LinearLayout languages = new LinearLayout(this);
        from = languages(languages, "原文语言", 0);
        button(languages, "⇄", () -> {
            int previous = from.getSelectedItemPosition();
            from.setSelection(to.getSelectedItemPosition());
            to.setSelection(previous);
            invalidate();
        });
        to = languages(languages, "译文语言", 1);
        root.addView(languages);
        ScrollView scroll = new ScrollView(this);
        LinearLayout content = new LinearLayout(this);
        content.setOrientation(LinearLayout.VERTICAL);
        content.addView(label("原文", 16));
        source = new EditText(this);
        source.setContentDescription("翻译原文");
        source.setHint("输入原文");
        source.setInputType(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_FLAG_MULTI_LINE
            | InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS);
        source.setPrivateImeOptions(EDITOR);
        source.setMinLines(3);
        source.setMaxLines(5);
        source.setSaveEnabled(false);
        source.setTextColor(0xff202124);
        source.setHintTextColor(0xff666666);
        content.addView(source, new LinearLayout.LayoutParams(-1, -2));
        LinearLayout inputActions = new LinearLayout(this);
        button(inputActions, "语音", this::voice);
        button(inputActions, "清空", () -> source.setText(""));
        translate = button(inputActions, "翻译", this::translate);
        content.addView(inputActions);
        content.addView(label("译文", 16));
        result = label("", 20);
        result.setContentDescription("翻译结果");
        result.setTextIsSelectable(true);
        result.setSaveEnabled(false);
        content.addView(result);
        status = label("", 14);
        content.addView(status);
        scroll.addView(content);
        root.addView(scroll, new LinearLayout.LayoutParams(-1, 0, 1));
        LinearLayout actions = new LinearLayout(this);
        copy = button(actions, "复制译文", () -> {
            if (draft.answer().isEmpty()) return;
            getSystemService(ClipboardManager.class).setPrimaryClip(ClipData.newPlainText("译文", draft.answer()));
            status.setText("译文已复制");
        });
        insert = button(actions, "插入", () -> {
            String text = draft.answer();
            if (text.isEmpty()) return;
            ResultReceiver receiver = getIntent().getParcelableExtra(RECEIVER);
            if (receiver == null) { status.setText("请从输入法的翻译入口打开，或复制译文"); return; }
            draft.consume();
            Bundle payload = new Bundle();
            payload.putString(TEXT, text);
            receiver.send(RESULT_OK, payload);
            insert.setEnabled(false);
            finish();
        });
        root.addView(actions);
        setContentView(root);
        ViewCompat.requestApplyInsets(root);
        source.addTextChangedListener(new TextWatcher() {
            @Override public void beforeTextChanged(CharSequence text, int start, int count, int after) { }
            @Override public void onTextChanged(CharSequence text, int start, int before, int count) { invalidate(); }
            @Override public void afterTextChanged(Editable text) { }
        });
        invalidate();
        String retained = getIntent().getStringExtra(TEXT);
        if (retained != null && !retained.isEmpty()) {
            long request = draft.invalidate();
            draft.complete(request, retained);
            showAnswer();
            status.setText("输入位置已变化，请复制译文");
        }
    }

    private Spinner languages(LinearLayout row, String label, int initial) {
        Spinner picker = new Spinner(this);
        picker.setContentDescription(label);
        picker.setAdapter(new ArrayAdapter<>(this, android.R.layout.simple_spinner_dropdown_item, LABELS));
        picker.setSelection(initial);
        picker.setOnItemSelectedListener(new AdapterView.OnItemSelectedListener() {
            private int previous = initial;
            @Override public void onItemSelected(AdapterView<?> parent, View view, int position, long id) {
                if (previous != position) { previous = position; invalidate(); }
            }
            @Override public void onNothingSelected(AdapterView<?> parent) { }
        });
        row.addView(picker, new LinearLayout.LayoutParams(0, -2, 1));
        return picker;
    }

    private TextView label(String text, int size) {
        TextView view = new TextView(this);
        view.setText(text);
        view.setTextSize(size);
        view.setTextColor(0xff202124);
        view.setPadding(0, 8, 0, 8);
        return view;
    }

    private Button button(LinearLayout row, String title, Runnable action) {
        Button button = new Button(this);
        button.setText(title);
        button.setOnClickListener(view -> action.run());
        row.addView(button);
        return button;
    }

    private void invalidate() {
        draft.invalidate();
        if (result == null) return;
        result.setText("");
        status.setText("");
        insert.setEnabled(false);
        copy.setEnabled(false);
    }

    private void showAnswer() {
        result.setText(draft.answer());
        boolean ready = !draft.answer().isEmpty();
        insert.setEnabled(ready && getIntent().hasExtra(RECEIVER));
        copy.setEnabled(ready);
    }

    /** 只在明确点击时读取配置并联网，不改变候选翻译开关或自动回落到另一家。 */
    private void translate() {
        if (busy) return;
        String text = source.getText().toString().strip();
        String sourceLanguage = CODES[from.getSelectedItemPosition()];
        String targetLanguage = CODES[to.getSelectedItemPosition()];
        long request = draft.invalidate();
        showAnswer();
        if (text.isEmpty() || text.codePointCount(0, text.length()) > UserTranslationClient.MAX_SOURCE_CODE_POINTS) {
            status.setText("请输入 1–256 字原文"); return;
        }
        if (sourceLanguage.equals(targetLanguage)) { status.setText("请选择不同的原文和译文语言"); return; }
        busy = true;
        translate.setEnabled(false);
        status.setText("正在翻译…");
        worker.execute(() -> {
            String answer = null;
            String failure = "翻译失败，请检查服务配置或稍后重试";
            try {
                JSONObject envelope = new JSONObject(NativeClient.loadPreferences(directory()));
                JSONObject snapshot = envelope.optBoolean("ok") ? envelope.optJSONObject("value") : null;
                JSONObject preferences = snapshot == null ? null : snapshot.optJSONObject("preferences");
                if (preferences != null) {
                    preferences = new JSONObject(preferences.toString()).put("candidate_translations", true);
                    CandidateTranslationRoute route = MSIMEInputService.candidateTranslationRouteFrom(preferences);
                    if (route.provider() == CandidateTranslationRoute.Provider.NONE) failure = "请先在输入设置中配置翻译服务";
                    else {
                        CandidateTranslationStore.Service service = route.provider() == CandidateTranslationRoute.Provider.ACCOUNT
                            ? new BackendTranslationClient(this, sourceLanguage) : new UserTranslationClient(route, sourceLanguage);
                        List<String> answers = service.translate(List.of(text), targetLanguage);
                        if (answers != null && !answers.isEmpty()) answer = answers.get(0);
                    }
                }
            } catch (Exception | LinkageError unavailable) { /* 不展示含凭据或原文的服务错误。 */ }
            final String translated = answer;
            final String message = failure;
            main.post(() -> {
                if (destroyed) return;
                busy = false;
                translate.setEnabled(true);
                if (draft.complete(request, translated)) { showAnswer(); status.setText(""); }
                else if (draft.current(request)) status.setText(message);
            });
        });
    }

    private String directory() {
        return HostOptionsPolicy.readOption(getFilesDir(), "preferences_directory");
    }

    /** 识别结果只进入原文，不自动触发翻译或插入聊天框。 */
    private void voice() {
        final long request = ++voiceGeneration;
        final String language = CODES[from.getSelectedItemPosition()];
        final String previous = source.getText().toString();
        worker.execute(() -> {
            String directory = directory();
            String id = "translation-" + UUID.randomUUID();
            VoiceConfiguration config = VoiceConfiguration.read(directory, id);
            main.post(() -> {
                if (destroyed || request != voiceGeneration) return;
                ResultReceiver receiver = new ResultReceiver(main) {
                    @Override protected void onReceiveResult(int code, Bundle data) {
                        if (destroyed || request != voiceGeneration || code != RESULT_OK || data == null) return;
                        voiceGeneration++;
                        try {
                            UUID resultId = UUID.fromString(data.getString(VoiceRecognitionActivity.RESULT_ID));
                            String recognized = new VoiceResultStore(getFilesDir().toPath().resolve("voice-handoff"))
                                .consume(resultId, System.currentTimeMillis());
                            if (previous.equals(source.getText().toString())) {
                                source.append(VoicePunctuation.apply(recognized, config.punctuationMode()));
                            } else status.setText("原文已变化，请重新录音");
                        } catch (VoiceResultStore.Failure | IllegalArgumentException | NullPointerException error) {
                            status.setText("无法领取语音结果，请重试");
                        }
                    }
                };
                try {
                    VoiceRecognitionActivity.launch(this, id, language, config.providerName(), config.requestFormat(),
                        config.endpoint(), config.model(), config.token(), config.streaming(), config.polish(),
                        config.localModel(), null, null, null, receiver);
                } catch (RuntimeException unavailable) {
                    VoiceRecognitionActivity.clearRequest(id);
                    status.setText("无法启动语音识别，请检查语音设置");
                }
            });
        });
    }

    @Override protected void onDestroy() {
        destroyed = true;
        voiceGeneration++;
        draft.invalidate();
        worker.shutdownNow();
        main.removeCallbacksAndMessages(null);
        super.onDestroy();
    }
}
