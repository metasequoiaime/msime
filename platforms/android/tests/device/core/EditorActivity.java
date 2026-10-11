package app.msime.android.test;

import android.app.Activity;
import android.content.ClipData;
import android.content.ClipboardManager;
import android.graphics.Bitmap;
import android.graphics.Color;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.text.InputType;
import android.view.WindowInsets;
import android.view.inputmethod.EditorInfo;
import android.view.inputmethod.InputConnection;
import android.view.inputmethod.InputConnectionWrapper;
import android.view.inputmethod.InputContentInfo;
import android.widget.Button;
import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import android.view.WindowInsetsController;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.TextView;
import android.content.Context;
import android.view.inputmethod.InputMethodManager;
import android.view.WindowManager;
import app.msime.android.WindowLayout;

/** Separate synthetic editor app: tests the system InputConnection, not a mock. */
public final class EditorActivity extends Activity {
    @SuppressWarnings("deprecation")
    @Override public void onCreate(Bundle state) {
        WindowLayout.theme(this);
        super.onCreate(state);
        getWindow().setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE
            | WindowManager.LayoutParams.SOFT_INPUT_STATE_ALWAYS_VISIBLE);
        LinearLayout layout = new LinearLayout(this);
        layout.setOrientation(LinearLayout.VERTICAL);
        WindowLayout.fitSystemBars(layout);
        TextView title = new TextView(this);
        title.setText("MSIME synthetic editor fixture");
        layout.addView(title);
        EditText plain = new EditText(this);
        plain.setInputType(InputType.TYPE_CLASS_TEXT);
        plain.setContentDescription("msime-test-plain");
        plain.setHint("Plain editor");
        showImeOnFocus(plain);
        layout.addView(plain);
        EditText password = new EditText(this);
        password.setInputType(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_PASSWORD);
        password.setContentDescription("msime-test-password");
        password.setHint("Password editor");
        showImeOnFocus(password);
        layout.addView(password);
        // 系统标记为邮箱的输入框：邮箱后缀（#6147）只在这种输入框里出现。
        EditText email = new EditText(this);
        email.setInputType(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS);
        email.setContentDescription("msime-test-email");
        email.setHint("Email editor");
        showImeOnFocus(email);
        layout.addView(email);
        EditText cursorAtEnd = new CursorAtEndEditText(this);
        cursorAtEnd.setInputType(InputType.TYPE_CLASS_TEXT);
        cursorAtEnd.setContentDescription("msime-test-cursor-at-end");
        cursorAtEnd.setHint("Editor that ignores newCursorPosition");
        showImeOnFocus(cursorAtEnd);
        layout.addView(cursorAtEnd);
        // 收图片的输入框（#6781）：经 contentMimeTypes 声明收 PNG 和 GIF，收到 commitContent 时读出图片，把类型、字节数和摘要写进下面的状态行。
        EditText image = new ImageEditText(this);
        image.setInputType(InputType.TYPE_CLASS_TEXT);
        image.setContentDescription("msime-test-image");
        image.setHint("Editor that accepts images");
        showImeOnFocus(image);
        layout.addView(image);
        // 把一张合成的 PNG 以本测试包的 content URI 放进系统剪贴板，相当于用户在别的应用里「复制图片」。按钮在触摸模式下不抢焦点，输入框和键盘保持不变。
        Button copyImage = new Button(this);
        copyImage.setText("Copy synthetic image");
        copyImage.setContentDescription("msime-test-copy-image");
        copyImage.setOnClickListener(view -> copySyntheticImage());
        layout.addView(copyImage);
        imageStatus = new TextView(this);
        imageStatus.setContentDescription("msime-test-image-status");
        imageStatus.setText("idle");
        layout.addView(imageStatus);
        EditText noCursorAnchor = new NoCursorAnchorEditText(this);
        noCursorAnchor.setInputType(InputType.TYPE_CLASS_TEXT);
        noCursorAnchor.setContentDescription("msime-test-no-cursor-anchor");
        noCursorAnchor.setHint("Editor that reports no cursor position");
        showImeOnFocus(noCursorAnchor);
        layout.addView(noCursorAnchor);
        // 窄的多行输入框：一段长组字放不下一行，折到下一行（#6792 候选栏跟随光标要放在组字最后一行下方）。
        EditText wrapping = new EditText(this);
        wrapping.setInputType(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_FLAG_MULTI_LINE);
        wrapping.setContentDescription("msime-test-wrapping");
        wrapping.setHint("Wrap");
        showImeOnFocus(wrapping);
        layout.addView(wrapping, new LinearLayout.LayoutParams(
            Math.round(120 * getResources().getDisplayMetrics().density), LinearLayout.LayoutParams.WRAP_CONTENT));
        setContentView(layout);
    }

    private TextView imageStatus;
    private int imeRequestGeneration;

    /** 合成图片在本测试包缓存里的位置；{@link ImageClipProvider} 只提供这一个文件。 */
    static File syntheticImage(Context context) {
        return new File(context.getCacheDir(), "synthetic-clip.png");
    }

    private void copySyntheticImage() {
        File file = syntheticImage(this);
        Bitmap bitmap = Bitmap.createBitmap(8, 8, Bitmap.Config.ARGB_8888);
        // 每次复制都换一个颜色，内容不同，摘要也不同。
        bitmap.eraseColor(Color.rgb((int) (System.nanoTime() & 0xff), 0x80, 0x40));
        try (FileOutputStream output = new FileOutputStream(file)) {
            if (!bitmap.compress(Bitmap.CompressFormat.PNG, 100, output)) throw new IOException("compress");
        } catch (IOException error) {
            imageStatus.setText("copy-failed");
            return;
        } finally {
            bitmap.recycle();
        }
        Uri uri = Uri.parse("content://" + ImageClipProvider.AUTHORITY + "/" + file.getName());
        ClipboardManager clipboard = getSystemService(ClipboardManager.class);
        clipboard.setPrimaryClip(ClipData.newUri(getContentResolver(), "msime synthetic image", uri));
        try (InputStream input = java.nio.file.Files.newInputStream(file.toPath())) {
            imageStatus.setText("copied:" + describe(input));
        } catch (IOException error) {
            imageStatus.setText("copy-failed");
        }
    }

    /** 字节数和 SHA-256 前 16 位，状态行里只放这些，不放图片本身。 */
    private static String describe(InputStream input) throws IOException {
        MessageDigest digest;
        try {
            digest = MessageDigest.getInstance("SHA-256");
        } catch (NoSuchAlgorithmException error) {
            throw new IOException(error);
        }
        byte[] buffer = new byte[8192];
        long total = 0;
        int read;
        while ((read = input.read(buffer)) != -1) {
            digest.update(buffer, 0, read);
            total += read;
        }
        StringBuilder hex = new StringBuilder();
        for (byte value : digest.digest()) hex.append(String.format("%02x", value));
        return total + ":" + hex.substring(0, 16);
    }

    /** 输入法经 commitContent 交来的图片：按需取读权限，读完就放掉，状态行写成 `received:<声明的类型>:<provider 报的类型>:<字节数>:<摘要>`。很多应用按 `ContentResolver.getType` 校验收到的内容，所以 provider 报的类型也要和声明的一致。 */
    private boolean receiveImage(InputContentInfo content, int flags) {
        boolean granted = (flags & InputConnection.INPUT_CONTENT_GRANT_READ_URI_PERMISSION) != 0;
        try {
            if (granted) content.requestPermission();
            String type = content.getDescription().getMimeTypeCount() > 0
                ? content.getDescription().getMimeType(0) : "";
            try (InputStream input = getContentResolver().openInputStream(content.getContentUri())) {
                if (input == null) throw new IOException("no stream");
                String served = getContentResolver().getType(content.getContentUri());
                imageStatus.setText("received:" + type + ":" + served + ":" + describe(input));
            }
            return true;
        } catch (IOException | SecurityException error) {
            imageStatus.setText("receive-failed:" + error.getClass().getSimpleName());
            return false;
        } finally {
            if (granted) content.releasePermission();
        }
    }

    private final class ImageEditText extends EditText {
        ImageEditText(Context context) { super(context); }

        @Override public InputConnection onCreateInputConnection(EditorInfo outAttrs) {
            InputConnection connection = super.onCreateInputConnection(outAttrs);
            outAttrs.contentMimeTypes = new String[] {"image/png", "image/gif"};
            if (connection == null) return null;
            return new InputConnectionWrapper(connection, false) {
                @Override public boolean commitContent(InputContentInfo content, int flags, Bundle options) {
                    return receiveImage(content, flags);
                }
            };
        }
    }

    /**
     * 不认 `commitText` 第二个参数的编辑器：不管输入法传多少，都当作 1，光标落在新文字后面。#6458 里 vivo「信息」和系统设置的搜索框就是这样，输入法用 `commitText(closing, 0)` 补上的后半个把光标带到了括号外面；这个输入框在模拟器上重现那种行为。
     */
    private static final class CursorAtEndEditText extends EditText {
        CursorAtEndEditText(Context context) { super(context); }

        @Override public InputConnection onCreateInputConnection(EditorInfo outAttrs) {
            InputConnection connection = super.onCreateInputConnection(outAttrs);
            if (connection == null) return null;
            return new InputConnectionWrapper(connection, false) {
                @Override public boolean commitText(CharSequence text, int newCursorPosition) {
                    return super.commitText(text, 1);
                }
            };
        }
    }

    /**
     * 不报告光标位置的编辑器：`requestCursorUpdates` 一律返回假，与自绘编辑器、部分 WebView 和游戏一样（#6792）。「物理键盘时候选栏跟随光标」在这种输入框里要退回停在底部的候选条。
     */
    private static final class NoCursorAnchorEditText extends EditText {
        NoCursorAnchorEditText(Context context) { super(context); }

        @Override public InputConnection onCreateInputConnection(EditorInfo outAttrs) {
            InputConnection connection = super.onCreateInputConnection(outAttrs);
            if (connection == null) return null;
            return new InputConnectionWrapper(connection, false) {
                @Override public boolean requestCursorUpdates(int cursorUpdateMode) {
                    return false;
                }

                // 安卓 13 起框架转给编辑器的是带过滤参数的这一个，包装类会直接交给被包装的连接，不经过上面那个。
                @Override public boolean requestCursorUpdates(int cursorUpdateMode, int cursorUpdateFilter) {
                    return false;
                }
            };
        }
    }

    private void showImeOnFocus(EditText editor) {
        editor.setOnFocusChangeListener((view, focused) -> {
            if (focused) requestIme(editor);
        });
    }

    @Override public void onWindowFocusChanged(boolean focused) {
        super.onWindowFocusChanged(focused);
        if (focused && getCurrentFocus() instanceof EditText editor) requestIme(editor);
    }

    private void requestIme(EditText editor) {
        int generation = ++imeRequestGeneration;
        if (Build.VERSION.SDK_INT >= 30) editor.setStateDescription("ime-requests-pending");
        for (long delay : new long[] {250, 750, 1500, 3000}) {
            editor.postDelayed(() -> {
                if (!editor.hasWindowFocus() || !editor.hasFocus()) return;
                InputMethodManager manager = getSystemService(InputMethodManager.class);
                if (manager != null)
                    manager.showSoftInput(editor, InputMethodManager.SHOW_IMPLICIT);
                if (Build.VERSION.SDK_INT >= 30) {
                    WindowInsetsController controller = editor.getWindowInsetsController();
                    if (controller != null) controller.show(WindowInsets.Type.ime());
                    // 最后一轮显示请求发出后，设备套件才开始注入第一颗实体键。
                    if (delay == 3000 && generation == imeRequestGeneration)
                        editor.setStateDescription("ime-requests-settled");
                }
            }, delay);
        }
    }
}
