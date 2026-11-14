package app.msime.android.test;

import org.json.JSONObject;

/** 在产品进程验证机器翻译的共享描述器与真正的键盘选路，不访问网络或修改偏好。 */
final class AliyunTranslationDeviceChecks {
    private AliyunTranslationDeviceChecks() {}

    private static Object route(ClassLoader loader, JSONObject preferences) throws Exception {
        return loader.loadClass("app.msime.android.MSIMEInputService")
            .getMethod("candidateTranslationRouteFrom", org.json.JSONObject.class).invoke(null, preferences);
    }

    /** 通过真正的 JNI 校验两版本、选路、空回答和失败，不访问付费服务。 */
    static void run(DeviceSmoke smoke) throws Exception {
        ClassLoader loader = smoke.getTargetContext().getClassLoader();
        Class<?> policy = loader.loadClass("app.msime.android.AliyunTranslationPolicy");
        Class<?> routeType = loader.loadClass("app.msime.android.CandidateTranslationRoute");
        var request = policy.getMethod("request", routeType, String.class, String.class);
        var parse = policy.getMethod("parse", byte[].class);
        var config = new org.json.JSONObject().put("enabled", true).put("access_key_id", "synthetic-id")
            .put("access_key_secret", "synthetic-secret").put("region", "cn-hangzhou")
            .put("edition", "general").put("scene", "social");
        var preferences = new org.json.JSONObject().put("candidate_translations", true)
            .put("translation_account", true).put("aliyun_translation", config);
        var route = route(loader, preferences);
        if (!provider(route).equals("ALIYUN") || !((Boolean) routeType.getMethod("userService").invoke(route)))
            throw new AssertionError("阿里云未独占选路");
        var descriptor = (JSONObject) request.invoke(null, route, "合成测试", "en");
        if (descriptor == null || !descriptor.getString("url").equals("https://mt.cn-hangzhou.aliyuncs.com/"))
            throw new AssertionError("通用版描述器错误");
        if (route.toString().contains("synthetic") || scope(route).contains("synthetic"))
            throw new AssertionError("路由泄露合成凭据");
        config.put("edition", "professional").put("scene", "medical").put("region", "cn-hongkong");
        var changed = route(loader, preferences);
        var professional = (JSONObject) request.invoke(null, changed, "合成测试", "en");
        if (professional == null || !professional.getJSONObject("headers").getString("x-acs-action").equals("Translate")
                || !professional.getString("body_utf8").contains("Scene=medical") || scope(changed).equals(scope(route)))
            throw new AssertionError("专业版配置未影响签名或缓存");
        for (String body : new String[] {"{\"Code\":10013,\"Data\":{\"Translated\":\"stale\"}}", "{\"Code\":200,\"Data\":{}}"}) {
            var parsed = (JSONObject) parse.invoke(null, (Object) body.getBytes(java.nio.charset.StandardCharsets.UTF_8));
            if (parsed == null || parsed.getBoolean("valid")) throw new AssertionError("错误回复被当作空成功");
        }
        var empty = (JSONObject) parse.invoke(null, (Object) "{\"Code\":200,\"Data\":{\"Translated\":\"\"}}".getBytes(java.nio.charset.StandardCharsets.UTF_8));
        if (empty == null || !empty.getBoolean("valid") || !empty.getString("translation").isEmpty())
            throw new AssertionError("合法空回答被当作失败");
        config.put("access_key_secret", "");
        if (!provider(route(loader, preferences)).equals("NONE"))
            throw new AssertionError("凭据不全回落到了其他服务");
        config.put("access_key_secret", "synthetic-secret"); preferences.put("candidate_translations", false);
        if (!provider(route(loader, preferences)).equals("NONE"))
            throw new AssertionError("总开关关闭后仍启用阿里云");
    }

    private static String provider(Object route) throws Exception {
        return route.getClass().getMethod("provider").invoke(route).toString();
    }

    private static String scope(Object route) throws Exception {
        return (String) route.getClass().getMethod("scope").invoke(route);
    }
}
