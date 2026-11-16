package app.msime.android.test;

import android.app.Activity;
import android.content.Context;
import android.net.Uri;
import android.os.Bundle;
import android.text.InputType;
import android.view.WindowManager;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.InputConnectionWrapper;
import android.view.inputmethod.InputContentInfo;
import android.view.inputmethod.InputMethodManager;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.TextView;
import app.msime.android.WindowLayout;
import java.io.IOException;
import java.io.InputStream;

/**
 * 表情包发送的合成编辑器：三个都不在 `EditorInfo.contentMimeTypes` 里声明图片类型的输入框，各自学一类真实应用。
 *
 * <p>状态行放在最上面、输入框紧随其后，键盘弹起后在任何分辨率下都留在可见区；{@link EditorActivity} 的状态行排在十来个输入框之后，在 Pixel 3 XL（1440x2960，density 560）上会被键盘挤出去。
 */
public final class StickerEditorActivity extends Activity {
    private TextView status;

    @SuppressWarnings("deprecation")
    @Override public void onCreate(Bundle state) {
        WindowLayout.theme(this);
        super.onCreate(state);
        getWindow().setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE);
        LinearLayout layout = new LinearLayout(this);
        layout.setOrientation(LinearLayout.VERTICAL);
        WindowLayout.fitSystemBars(layout);
        status = new TextView(this);
        status.setContentDescription("msime-test-sticker-status");
        status.setText("idle");
        layout.addView(status);
        add(layout, new UndeclaredImageEditText(this), "msime-test-sticker-undeclared");
        add(layout, new DiscardingEditText(this), "msime-test-sticker-discarding");
        add(layout, new ExpressionEditText(this), "msime-test-sticker-expression");
        setContentView(layout);
    }

    private void add(LinearLayout layout, EditText editor, String description) {
        editor.setInputType(InputType.TYPE_CLASS_TEXT);
        editor.setContentDescription(description);
        editor.setHint(description);
        editor.setSingleLine(true);
        editor.setOnFocusChangeListener((view, focused) -> {
            if (focused) view.post(() -> getSystemService(InputMethodManager.class)
                .showSoftInput(view, InputMethodManager.SHOW_IMPLICIT));
        });
        layout.addView(editor);
    }

    /** 不声明图片类型、但 `commitContent` 真的把图读走的输入框：状态行写成 `received:<声明的类型>:<provider 报的类型>:<字节数>:<摘要>`。 */
    private final class UndeclaredImageEditText extends EditText {
        UndeclaredImageEditText(Context context) { super(context); }

        @Override public InputConnection onCreateInputConnection(EditorInfo outAttrs) {
            InputConnection connection = super.onCreateInputConnection(outAttrs);
            if (connection == null) return null;
            return new InputConnectionWrapper(connection, false) {
                @Override public boolean commitContent(InputContentInfo content, int flags, Bundle options) {
                    boolean granted = (flags & InputConnection.INPUT_CONTENT_GRANT_READ_URI_PERMISSION) != 0;
                    try {
                        if (granted) content.requestPermission();
                        String type = content.getDescription().getMimeTypeCount() > 0
                            ? content.getDescription().getMimeType(0) : "";
                        try (InputStream input = getContentResolver().openInputStream(content.getContentUri())) {
                            if (input == null) throw new IOException("no stream");
                            status.setText("received:" + type + ":" + getContentResolver().getType(content.getContentUri())
                                + ":" + EditorActivity.describe(input));
                        }
                        return true;
                    } catch (IOException | SecurityException error) {
                        status.setText("receive-failed:" + error.getClass().getSimpleName());
                        return false;
                    } finally {
                        if (granted) content.releasePermission();
                    }
                }
            };
        }
    }

    /** 不声明图片类型、`commitContent` 返回 true 却把内容丢掉的输入框：真机上 Flutter 应用的文本框表现如此，输入法从返回值看不出图片没进去。状态行只记下被丢掉的次数。 */
    private final class DiscardingEditText extends EditText {
        private int discarded;

        DiscardingEditText(Context context) { super(context); }

        @Override public InputConnection onCreateInputConnection(EditorInfo outAttrs) {
            InputConnection connection = super.onCreateInputConnection(outAttrs);
            if (connection == null) return null;
            return new InputConnectionWrapper(connection, false) {
                @Override public boolean commitContent(InputContentInfo content, int flags, Bundle options) {
                    status.setText("discarded:" + ++discarded);
                    return true;
                }
            };
        }
    }

    /** 微信聊天输入框的做法（extras 里写 `SUPPORT_SOGOU_EXPRESSION` = 1，经私有命令收表情），外加一个返回 true 却丢内容的 `commitContent`：输入法若先试 `commitContent`，私有命令就不会再发，状态行会停在 `discarded-before-expression`。 */
    private final class ExpressionEditText extends EditText {
        ExpressionEditText(Context context) { super(context); }

        @Override public InputConnection onCreateInputConnection(EditorInfo outAttrs) {
            InputConnection connection = super.onCreateInputConnection(outAttrs);
            if (outAttrs.extras == null) outAttrs.extras = new Bundle();
            outAttrs.extras.putInt("SUPPORT_SOGOU_EXPRESSION", 1);
            if (connection == null) return null;
            return new InputConnectionWrapper(connection, false) {
                @Override public boolean commitContent(InputContentInfo content, int flags, Bundle options) {
                    status.setText("discarded-before-expression");
                    return true;
                }
            };
        }

        // 带类型的 getParcelable 要 API 33，这个编辑器也跑在 API 28 的 AVD 上。
        @SuppressWarnings("deprecation")
        @Override public boolean onPrivateIMECommand(String action, Bundle data) {
            if (!"com.sogou.inputmethod.exp.commit".equals(action) || data == null) return super.onPrivateIMECommand(action, data);
            Uri uri = data.getParcelable("EXP_PATH_URI");
            if (uri == null) return false;
            try (InputStream input = getContentResolver().openInputStream(uri)) {
                if (input == null) throw new IOException("no stream");
                status.setText("expression:" + getContentResolver().getType(uri) + ":" + EditorActivity.describe(input));
                return true;
            } catch (IOException | SecurityException error) {
                status.setText("expression-failed:" + error.getClass().getSimpleName());
                return false;
            }
        }
    }
}
