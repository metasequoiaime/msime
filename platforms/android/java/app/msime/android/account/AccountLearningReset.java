package app.msime.android;

import android.content.Context;
import org.json.JSONException;
import org.json.JSONObject;

/** 清除时绑定本机的具体登录；屏障和清除均由共享 Rust 在同一维护边界执行。 */
final class AccountLearningReset {
    private AccountLearningReset() {}

    static JSONObject reset(Context context, JSONObject options)
            throws AccountStore.Failure, JSONException {
        JSONObject binding = RustAccount.accountObject(context,
            new JSONObject().put("operation", "learning_binding"));
        Object login = binding.opt("login");
        if (login == JSONObject.NULL) {
            Object value = AccountStore.value(NativeClient.dictionary(new JSONObject()
                .put("options", options)
                .put("action", new JSONObject().put("operation", "reset")).toString()));
            if (value instanceof JSONObject object) return object;
            throw new AccountStore.Failure("learning_local_unavailable", 0);
        }
        // 登录读取失败、形状错误或账号已经切换时不能退回无屏障的旧清除。
        if (!(login instanceof JSONObject))
            throw new AccountStore.Failure("account_unavailable", 0);
        return RustAccount.accountObject(context, new JSONObject()
            .put("operation", "learning_reset_local").put("login", login).put("options", options));
    }
}
