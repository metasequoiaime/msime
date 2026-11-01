package app.msime.android.home;

import android.Manifest;
import android.content.Context;
import android.content.pm.PackageManager;
import android.database.Cursor;
import android.provider.ContactsContract;
import android.widget.LinearLayout;
import androidx.activity.result.ActivityResultLauncher;
import androidx.activity.result.contract.ActivityResultContracts;
import androidx.annotation.Nullable;
import app.msime.android.JsonPolicy;
import app.msime.android.NativeClient;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/** 联系人导入页：只有用户点击导入时申请权限，名单保存到共享 mentions 存储。 */
public final class ContactsPage extends ReloadingDetailPage {
    private static final class Contact {
        final String name;
        final String phone;
        Contact(String name, String phone) { this.name = name; this.phone = phone; }
    }
    @Override protected List<String> sectionTitles() { return List.of("联系人"); }

    private final ActivityResultLauncher<String> permission = registerForActivityResult(
        new ActivityResultContracts.RequestPermission(), granted -> {
            if (granted) importContacts();
            else showMessage("没有联系人权限，未导入电话本");
        });

    @Override protected void reload() {
        HostTask.run(this, ContactsPage::load, this::render);
    }

    @Nullable private static List<Contact> load(Context context) {
        try {
            JSONObject request = request(context, new JSONObject().put("operation", "load_mentions"));
            JSONArray entries = JsonPolicy.arrayValue(request);
            if (entries == null) return null;
            List<Contact> result = new ArrayList<>();
            for (int i = 0; i < entries.length(); i++) {
                JSONObject entry = entries.optJSONObject(i);
                if (entry != null) result.add(new Contact(entry.optString("text", ""),
                    entry.optString("phone", "")));
            }
            return result;
        } catch (JSONException | RuntimeException | LinkageError error) {
            return null;
        }
    }

    private void render(@Nullable List<Contact> entries) {
        LinearLayout target = contentColumn();
        if (target == null) return;
        target.removeAllViews();
        GroupCard card = GroupCard.add(sectionColumn(target, "联系人"), null).withDividers(16);
        card.note(entries == null ? "联系人名单读取失败，请稍后重试。"
            : "已保存 " + entries.size() + " 个联系人。名单供输入法本地匹配，数据只保存在本机。");
        card.button("导入联系人", null, "导入", this::requestImport).help("首次点击时才申请读取联系人权限");
        card.button("刷新联系人", "重新读取电话本并替换本机名单", "刷新", this::requestImport);
        card.button("清空联系人", "删除已导入的联系人，不影响系统电话本", "清空", this::clearContacts);
    }

    private void requestImport() {
        if (android.os.Build.VERSION.SDK_INT >= 23 && requireContext().checkSelfPermission(Manifest.permission.READ_CONTACTS)
                != PackageManager.PERMISSION_GRANTED) {
            permission.launch(Manifest.permission.READ_CONTACTS);
        } else importContacts();
    }

    private void importContacts() {
        HostTask.run(this, ContactsPage::readContacts, result -> {
            if (result == null) showMessage("读取联系人失败，请稍后重试");
            else if (save(requireContext(), result)) { showMessage("已导入 " + result.size() + " 个联系人"); reload(); }
            else showMessage("联系人名单保存失败，请稍后重试");
        });
    }

    private static List<Contact> readContacts(Context context) {
        LinkedHashMap<String, String> values = new LinkedHashMap<>();
        String[] projection = {ContactsContract.CommonDataKinds.Phone.DISPLAY_NAME,
            ContactsContract.CommonDataKinds.Phone.NUMBER};
        try (Cursor cursor = context.getContentResolver().query(
                ContactsContract.CommonDataKinds.Phone.CONTENT_URI, projection, null, null,
                ContactsContract.CommonDataKinds.Phone.DISPLAY_NAME + " COLLATE LOCALIZED")) {
            if (cursor == null) return List.of();
            while (cursor.moveToNext() && values.size() < 1000) {
                String name = cursor.getString(0);
                String number = cursor.getString(1);
                if (name == null || name.isBlank()) continue;
                values.putIfAbsent(name.trim(), number == null ? "" : number.trim());
            }
        }
        List<Contact> entries = new ArrayList<>(values.size());
        for (var entry : values.entrySet())
            entries.add(new Contact(entry.getKey(), entry.getValue()));
        return entries;
    }

    private void clearContacts() {
        HostTask.run(this, context -> save(context, List.of()), ok -> {
            if (ok) { showMessage("已清空联系人"); reload(); }
            else showMessage("联系人名单清空失败，请稍后重试");
        });
    }

    private static boolean save(Context context, List<Contact> entries) {
        try {
            JSONArray values = new JSONArray();
            for (Contact entry : entries) values.put(new JSONObject().put("text", entry.name)
                .put("key", "").put("phone", entry.phone));
            JSONObject action = new JSONObject().put("operation", "save_mentions").put("entries", values);
            JSONObject response = request(context, action);
            return JsonPolicy.strictTrue(response.opt("ok"));
        } catch (JSONException | RuntimeException | LinkageError error) {
            return false;
        }
    }

    private static JSONObject request(Context context, JSONObject action) throws JSONException {
        JSONObject request = new JSONObject().put("state_root", HostStore.directory(context))
            .put("sound_packs", JSONObject.NULL).put("action", action);
        return new JSONObject(NativeClient.plugins(request.toString()));
    }

    private void showMessage(String text) {
        if (getContext() != null) MsToast.show(requireContext(), text);
    }
}
