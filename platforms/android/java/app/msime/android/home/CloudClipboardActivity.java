package app.msime.android.home;

import android.content.ClipData;
import android.content.ClipboardManager;
import android.graphics.drawable.GradientDrawable;
import android.os.Bundle;
import android.util.TypedValue;
import android.view.View;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.annotation.Nullable;
import androidx.appcompat.app.AppCompatActivity;
import androidx.core.content.ContextCompat;
import androidx.core.graphics.Insets;
import androidx.core.view.ViewCompat;
import androidx.core.view.WindowCompat;
import androidx.core.view.WindowInsetsCompat;
import com.google.android.material.appbar.MaterialToolbar;
import com.google.android.material.button.MaterialButton;
import com.google.android.material.switchmaterial.SwitchMaterial;
import com.google.android.material.textfield.TextInputEditText;
import app.msime.android.BackendAccount;
import app.msime.android.clipboard.CloudClipboardTextPolicy;
import app.msime.android.CloudClipboardPanelPolicy;
import app.msime.android.R;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/** Account-owned cloud clipboard. It never reads the Android clipboard automatically. */
public final class CloudClipboardActivity extends AppCompatActivity {
    private final ExecutorService worker = Executors.newSingleThreadExecutor();
    private SwitchMaterial enabled;
    private TextInputEditText search;
    private TextInputEditText draft;
    private LinearLayout items;
    private TextView status;
    private boolean busy;
    private boolean loaded;

    @Override protected void onCreate(@Nullable Bundle state) {
        AppMode.restore(this);
        super.onCreate(state);
        WindowCompat.setDecorFitsSystemWindows(getWindow(), false);
        setContentView(R.layout.activity_cloud_clipboard);
        View root = findViewById(R.id.cloud_clipboard_root);
        ViewCompat.setOnApplyWindowInsetsListener(root, (view, insets) -> {
            Insets bars = insets.getInsets(WindowInsetsCompat.Type.systemBars());
            view.setPadding(bars.left, bars.top, bars.right, bars.bottom);
            return insets;
        });
        MaterialToolbar toolbar = findViewById(R.id.cloud_clipboard_bar);
        toolbar.setNavigationOnClickListener(ignored -> finish());
        enabled = findViewById(R.id.cloud_clipboard_enabled);
        search = findViewById(R.id.cloud_clipboard_search);
        draft = findViewById(R.id.cloud_clipboard_draft);
        items = findViewById(R.id.cloud_clipboard_items);
        status = findViewById(R.id.cloud_clipboard_status);
        enabled.setOnCheckedChangeListener((button, checked) -> {
            if (!button.isPressed() || !CloudClipboardPanelPolicy.canMutate(loaded, busy)) return;
            run(() -> { new BackendAccount(this).setClipboardEnabled(checked); return null; });
        });
        findViewById(R.id.cloud_clipboard_refresh).setOnClickListener(ignored -> reload());
        findViewById(R.id.cloud_clipboard_add).setOnClickListener(ignored -> add());
        reload();
    }

    private void reload() {
        if (busy) return;
        invalidateLoadedState();
        String query = search == null || search.getText() == null ? "" : search.getText().toString();
        run(() -> new BackendAccount(this).clipboard(query), this::render);
    }

    private void add() {
        if (!CloudClipboardPanelPolicy.canMutate(loaded, busy)) return;
        String text = draft.getText() == null ? "" : draft.getText().toString();
        if (!CloudClipboardTextPolicy.valid(text)) {
            status.setText("请输入有效且不超过 4,000 个 UTF-16 单元的内容");
            return;
        }
        run(() -> { new BackendAccount(this).addClipboard(text); return null; }, ignored -> {
            draft.setText("");
            reload();
        });
    }

    private void render(BackendAccount.ClipboardPage page) {
        loaded = true;
        enabled.setEnabled(true);
        draft.setEnabled(true);
        enabled.setChecked(page.enabled());
        items.removeAllViews();
        if (page.items().isEmpty()) {
            TextView empty = new TextView(this);
            empty.setText(search.getText() == null || search.getText().length() == 0
                ? "还没有保存任何内容" : "没有匹配的内容");
            empty.setPadding(0, dp(16), 0, dp(16));
            empty.setTextColor(ContextCompat.getColor(this, R.color.text_secondary));
            items.addView(empty);
        }
        int count = page.items().size();
        for (int index = 0; index < count; index++) {
            BackendAccount.ClipboardItem item = page.items().get(index);
            // The design's record rows: 15sp text in one grouped surface card, a hairline between rows, rather than a stack of filled accent buttons.
            if (index > 0) {
                View divider = new View(this);
                divider.setBackgroundColor(ContextCompat.getColor(this, R.color.hairline));
                LinearLayout.LayoutParams line =
                    new LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, dp(1));
                divider.setLayoutParams(line);
                // The divider sits on the card's colour so the group reads as one piece.
                LinearLayout holder = new LinearLayout(this);
                holder.setBackgroundColor(ContextCompat.getColor(this, R.color.surface));
                holder.setPaddingRelative(dp(16), 0, 0, 0);
                holder.addView(divider);
                items.addView(holder);
            }
            TextView row = new TextView(this);
            row.setText(item.text());
            row.setTextSize(15);
            row.setTextColor(ContextCompat.getColor(this, R.color.ink));
            row.setMaxLines(3);
            row.setEllipsize(android.text.TextUtils.TruncateAt.END);
            row.setMinHeight(dp(52));
            row.setGravity(android.view.Gravity.START | android.view.Gravity.CENTER_VERTICAL);
            row.setPadding(dp(16), dp(12), dp(16), dp(12));
            row.setBackground(group(index == 0, index == count - 1));
            TypedValue ripple = new TypedValue();
            getTheme().resolveAttribute(android.R.attr.selectableItemBackground, ripple, true);
            row.setForeground(ContextCompat.getDrawable(this, ripple.resourceId));
            row.setContentDescription(item.text() + "，点按复制，长按删除");
            row.setOnClickListener(ignored -> {
                ClipboardManager clipboard = getSystemService(ClipboardManager.class);
                if (clipboard != null) clipboard.setPrimaryClip(ClipData.newPlainText("水杉云剪贴板", item.text()));
                status.setText("已复制到系统剪贴板");
            });
            row.setOnLongClickListener(ignored -> {
                if (!CloudClipboardPanelPolicy.canMutate(loaded, busy)) return true;
                run(() -> { new BackendAccount(this).deleteClipboard(item.id()); return null; }, ignored2 -> reload());
                return true;
            });
            items.addView(row);
        }
        if (!page.items().isEmpty()) {
            MaterialButton clear = new MaterialButton(this);
            clear.setText("清空历史");
            LinearLayout.LayoutParams gap = new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.WRAP_CONTENT, LinearLayout.LayoutParams.WRAP_CONTENT);
            gap.topMargin = dp(12);
            clear.setLayoutParams(gap);
            clear.setOnClickListener(ignored -> {
                if (!CloudClipboardPanelPolicy.canMutate(loaded, busy)) return;
                run(
                () -> { new BackendAccount(this).deleteClipboard(null); return null; },
                ignored2 -> reload());
            });
            items.addView(clear);
        }
        status.setText("最多保存 50 条；点按复制，长按删除");
    }

    private int dp(int value) {
        return Math.round(value * getResources().getDisplayMetrics().density);
    }

    /** The slice of the grouped card behind one record: rounded where the group starts and ends. */
    private GradientDrawable group(boolean first, boolean last) {
        float radius = dp(20);
        float top = first ? radius : 0f;
        float bottom = last ? radius : 0f;
        GradientDrawable card = new GradientDrawable();
        card.setColor(ContextCompat.getColor(this, R.color.surface));
        card.setCornerRadii(new float[] {top, top, top, top, bottom, bottom, bottom, bottom});
        return card;
    }

    private interface Work<T> { T run() throws Exception; }
    private <T> void run(Work<T> work) { run(work, ignored -> reload()); }
    private <T> void run(Work<T> work, java.util.function.Consumer<T> done) {
        if (busy) return;
        busy = true;
        status.setText("处理中…");
        worker.execute(() -> {
            try {
                T result = work.run();
                runOnUiThread(() -> {
                    if (isFinishing() || isDestroyed()) return;
                    busy = false;
                    done.accept(result);
                });
            } catch (Exception error) {
                runOnUiThread(() -> {
                    if (isFinishing() || isDestroyed()) return;
                    busy = false;
                    invalidateLoadedState();
                    status.setText("连接未完成，请登录后重试");
                });
            }
        });
    }

    /** Drop rows and controls whose account state was not confirmed by the latest fetch. */
    private void invalidateLoadedState() {
        loaded = false;
        if (enabled != null) {
            enabled.setEnabled(false);
            enabled.setChecked(false);
        }
        if (draft != null) draft.setEnabled(false);
        if (items != null) items.removeAllViews();
    }

    @Override protected void onDestroy() {
        worker.shutdownNow();
        super.onDestroy();
    }
}
