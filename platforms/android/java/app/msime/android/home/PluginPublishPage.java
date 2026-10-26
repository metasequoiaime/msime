package app.msime.android.home;

import android.content.Context;
import android.os.Bundle;
import android.text.InputType;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.annotation.NonNull;
import androidx.annotation.Nullable;
import app.msime.android.DimensionPolicy;
import app.msime.android.DrawablePolicy;
import app.msime.android.LayoutPolicy;
import app.msime.android.ThemeColorPolicy;
import app.msime.android.ViewPolicy;
import app.msime.android.policy.PluginCatalogPolicy;
import app.msime.android.policy.PluginEditorPolicy;
import java.util.ArrayList;
import java.util.List;
import java.util.UUID;

/**
 * 发布我的插件：选一个自己导入或制作的包，预检、填名称和说明、确认发布权利，公开发布到社区，与共享设置页 `community-plugin-publish-dialog.tsx` 同样的内容和文字。
 *
 * <p>预检和打包都在 client-services 里：`plugin_pack_preview` 按分享规则检查已装的包并算出归档大小（不上传），`plugin_publish` 打包上传（要登录）。发布 id 由这一页随机生成，同一次发布失败重试时沿用，服务端据此去重；换一个包就换一个 id。只列手机上生效的、不是内置的包；特效包、音乐包和按键旋律在电脑上发布。
 */
public final class PluginPublishPage extends ReloadingDetailPage {
    private static final String PUBLISH_WARNING =
        "上传的是插件目录中的全部文件（不含隐藏文件），压缩后不超过 8 MB；每个账号最多发布 20 个、合计 32 MB，每小时最多发布 10 次。其他用户下载后会按原样安装。";

    @Nullable private List<PluginCatalogPolicy.Pack> options;
    @Nullable private PluginCatalogPolicy.Pack chosen;
    @Nullable private PluginStore.PackPreview preview;
    private String packError = "";
    private boolean packLoading;
    private String name = "";
    private String description = "";
    private boolean agreed;
    private String publication = UUID.randomUUID().toString();
    private boolean busy;
    private String error = "";
    private boolean signIn;
    private long generation;
    @Nullable private TextView publishButton;

    @Override public void onCreate(@Nullable Bundle saved) {
        super.onCreate(saved);
        if (saved != null) publication = saved.getString("plugin-publication", publication);
    }

    @Override public void onSaveInstanceState(@NonNull Bundle out) {
        super.onSaveInstanceState(out);
        out.putString("plugin-publication", publication);
    }

    @Override protected void reload() {
        HostTask.run(this, context -> PluginStore.catalog(context), catalog -> {
            List<PluginCatalogPolicy.Pack> loaded = new ArrayList<>();
            if (catalog != null) {
                for (PluginStore.Entry entry : catalog.entries()) {
                    PluginCatalogPolicy.Pack pack = entry.pack();
                    if (!pack.builtin() && PluginCatalogPolicy.usedOnPhone(pack)) loaded.add(pack);
                }
            }
            options = catalog == null ? null : loaded;
            if (catalog == null) error = "读取本地插件失败，请重试。";
            if (chosen == null || !loaded.contains(chosen)) choose(loaded.isEmpty() ? null : loaded.get(0));
            else render();
        });
    }

    /** 回到这一页（例如登录回来）不重读，免得填好的名称和说明被预检的建议值换掉。 */
    @Override protected boolean shouldReloadWhenVisible() {
        return false;
    }

    private void choose(@Nullable PluginCatalogPolicy.Pack pack) {
        long current = ++generation;
        chosen = pack;
        preview = null;
        packError = "";
        agreed = false;
        publication = UUID.randomUUID().toString();
        packLoading = pack != null;
        render();
        if (pack == null) return;
        HostTask.run(this, context -> PluginStore.packPreview(context, pack.kind(), pack.id()), result -> {
            if (current != generation) return;
            packLoading = false;
            if (result == null || !result.succeeded()) {
                packError = result == null ? PluginCatalogPolicy.communityMessage("", "", false)
                    : PluginCatalogPolicy.communityMessage(result.code(), result.detail(), false);
            } else {
                preview = result.value();
                String suggested = preview.suggestedName();
                int limit = Math.min(suggested.length(),
                    suggested.offsetByCodePoints(0, Math.min(suggested.codePointCount(0, suggested.length()),
                        PluginCatalogPolicy.MAX_PUBLISH_NAME)));
                name = suggested.substring(0, limit);
                description = preview.suggestedDescription();
            }
            render();
        });
    }

    private static String optionLabel(PluginCatalogPolicy.Pack pack) {
        return PluginCatalogPolicy.packKindLabel(pack) + " · " + (pack.name().equals(pack.id()) ? pack.id()
            : pack.name() + "（" + pack.id() + "）");
    }

    private void render() {
        LinearLayout target = contentColumn();
        if (target == null) return;
        target.removeAllViews();
        publishButton = null;
        Context context = requireContext();
        GroupCard choice = GroupCard.add(target, null);
        if (options == null) {
            choice.note(error.isEmpty() ? "正在读取本地插件…" : error);
            return;
        }
        if (options.isEmpty()) {
            choice.note("还没有可发布的插件。内置插件不能发布，请先在「我的插件」中导入或制作自己的音效包、指令表、短语表、辅助码表、单词本或符号集。");
            return;
        }
        choice.nav("发布插件", null, chosen == null ? null : optionLabel(chosen), this::pickPack, true);
        if (packLoading) choice.note("正在检查插件…");
        if (!packError.isEmpty()) choice.note(packError);
        PluginStore.PackPreview current = preview;
        if (current == null) return;
        choice.note("v" + current.version() + (current.license().isEmpty() ? "" : " · 授权 " + current.license()) + " · "
            + current.fileCount() + " 个文件 · " + PluginCatalogPolicy.megabytes(current.size()) + " / 8 MB");

        GroupCard form = GroupCard.add(target, null);
        LinearLayout fields = LayoutPolicy.column(context);
        ViewPolicy.setPaddingDp(context, fields, Ui.ROW_PADDING_H, 4, Ui.ROW_PADDING_H, 12);
        form.addView(fields);
        field(fields, "名称", name, false, value -> name = value);
        field(fields, "说明", description, true, value -> description = value);
        form.toggle("我拥有插件中音频与文字的发布权利，并同意其他用户按包内授权免费下载使用", null, agreed, checked -> {
            agreed = checked;
            refresh();
        });
        form.note(PUBLISH_WARNING);

        GroupCard actions = GroupCard.add(target, null);
        if (!error.isEmpty()) actions.note(error);
        if (signIn) actions.button("登录水杉账号", null, "登录", this::signIn);
        TextView button = KeyboardSheets.bigButton(context, busy ? "正在发布…" : "公开发布", true, this::submit);
        LinearLayout.LayoutParams params = LayoutPolicy.matchWidthWrapParams();
        int margin = DimensionPolicy.pixels(context, Ui.ROW_PADDING_H);
        params.setMargins(margin, margin, margin, margin);
        actions.card().addView(button, params);
        publishButton = button;
        refresh();
    }

    private boolean ready() {
        return preview != null && !packLoading && !busy && agreed && PluginCatalogPolicy.publishNameValid(name)
            && PluginCatalogPolicy.publishDescriptionValid(description);
    }

    private void refresh() {
        if (publishButton == null) return;
        boolean ready = ready();
        publishButton.setEnabled(ready);
        publishButton.setAlpha(ready ? 1f : 0.45f);
    }

    private void field(LinearLayout parent, String label, String value, boolean multiline,
            java.util.function.Consumer<String> onChange) {
        Context context = requireContext();
        TextView caption = ViewPolicy.styledLabel(context, label, 13, 400, ThemeColorPolicy.subText(context));
        LinearLayout.LayoutParams captionParams = LayoutPolicy.matchWidthWrapParams();
        captionParams.topMargin = DimensionPolicy.pixels(context, 8);
        parent.addView(caption, captionParams);
        EditText input = ViewPolicy.styledInput(context, 15, 400, ThemeColorPolicy.text(context));
        input.setText(value);
        input.setInputType(InputType.TYPE_CLASS_TEXT | (multiline ? InputType.TYPE_TEXT_FLAG_MULTI_LINE : 0));
        if (multiline) {
            ViewPolicy.setTopStart(input);
            ViewPolicy.setMinLines(input, 3);
        } else {
            ViewPolicy.setSingleLine(input);
        }
        input.setContentDescription(label);
        ViewPolicy.setBackground(input, DrawablePolicy.outlined(ThemeColorPolicy.rowBackground(context),
            DimensionPolicy.pixels(context, 10), DimensionPolicy.atLeastOnePixel(context, 1), ThemeColorPolicy.hairline(context)));
        ViewPolicy.setSymmetricPaddingDp(context, input, 12, 8);
        input.setMinHeight(DimensionPolicy.pixels(context, 44));
        LinearLayout.LayoutParams params = LayoutPolicy.matchWidthWrapParams();
        params.topMargin = DimensionPolicy.pixels(context, 4);
        parent.addView(input, params);
        ViewPolicy.afterTextChanged(input, text -> {
            onChange.accept(text.toString());
            refresh();
        });
    }

    private void pickPack() {
        List<PluginCatalogPolicy.Pack> current = options;
        if (current == null || busy) return;
        OptionSheet sheet = new OptionSheet(requireContext(), "发布插件", "内置插件不能发布");
        for (PluginCatalogPolicy.Pack pack : current) {
            sheet.option(optionLabel(pack), pack.equals(chosen), () -> {
                if (!pack.equals(chosen)) choose(pack);
            });
        }
        sheet.show();
    }

    private void submit() {
        PluginCatalogPolicy.Pack pack = chosen;
        if (pack == null || !ready()) return;
        busy = true;
        error = "";
        signIn = false;
        render();
        String id = publication;
        String publishedName = PluginEditorPolicy.trim(name);
        String publishedDescription = PluginEditorPolicy.trim(description);
        HostTask.runNetwork(this, context -> PluginStore.publish(context, pack.kind(), pack.id(), id, publishedName,
            publishedDescription), result -> {
                busy = false;
                if (result == null || !result.succeeded()) {
                    String code = result == null ? "" : result.code();
                    error = PluginCatalogPolicy.communityMessage(code, result == null ? "" : result.detail(), true);
                    signIn = PluginCatalogPolicy.needsSignIn(code);
                    render();
                    return;
                }
                PluginsPage.communityChanged();
                MsToast.show(requireContext(), "已发布到社区。");
                requireActivity().getOnBackPressedDispatcher().onBackPressed();
            });
    }

    private void signIn() {
        SignIn.start(requireActivity(), failure -> {
            if (getView() == null) return;
            if (failure != null && !failure.isEmpty() && !LoginSheet.CANCELLED.equals(failure)) {
                MsToast.show(requireContext(), failure);
            }
            error = "";
            signIn = false;
            render();
        });
    }
}
