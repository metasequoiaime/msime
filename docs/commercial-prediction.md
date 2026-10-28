# 商业云候选与后续词预测

本次把商业输入法的“正在输入时的云候选”和“上屏后的后续词预测”分开处理。百度拼音云候选已接入；后续词预测在 Android/macOS 复用共享引擎，并支持另行授权的自定义云服务。搜狗原生私有云尚未直连。

## 样本与可确认结果

| 样本 | 字节 | SHA-256 |
| --- | ---: | --- |
| `pinyin_guanwang_16.9b.exe` | 224203120 | `0288486a5b9c248536d765e8f540b265b41754ca0be9f4680b2f37b3b35c3a81` |
| `sogou_mac_625b_new_guanwang.zip` | 186327683 | `581cc88c07fd2ef7e48d42a8bbd1adaba43e471bc5e64b2fc614385e48e4ef51` |
| `SogouInput_20.18.1_android_sweb.apk` | 98659313 | `079dc8fc43c073af2d40526aa82fd36242b07eaec8e2f186d8bfd3ce4874e84a` |

Windows 安装包经 Docker 的 7z 静态列出 325 个文件，其中含独立的 `SogouCloud.exe`、TSF DLL、Broker 和云转换资源；未运行安装程序。macOS 外层安装包中含嵌套的 `SogouInput.zip`；输入法核心资源包含 `sgim_gd_asso_bs.bin`、`sgim_gd_asso_bt.bin`、`sgim_gd_asscloud.bin`，以及独立的服务/任务管理器。二进制字符串包含 `WordPrediction`、`AssoPrefetch`、`CoreInitParam.cloud_input_url` 和 `cloud_websocket_url`。这些证据说明存在不同的本地/云联想模块，不能据此声称已恢复算法、模型或预测质量。

Android APK 通过 Docker 中的 JADX 1.5.6 静态解析，未执行其应用代码。`CloudInputInputRequestInfo` 和 `CloudAssocInputRequestInfo` 分别组装输入中与上屏后请求；后者持有独立请求身份、前后文、不同联想类型及回包处理。`CloudInputNetworkImpl` 将 Protobuf 请求交给 `SogouUrlEncrypt`，通过响应解密后再解析；网络地址由 `NetWorkSettingInfoManager` 按请求类型与远端配置选择。还观察到 `wss://ws-keyboard.shouji.sogou.com/ws/cloud_assoc`。这是一条加密业务协议，不能把任意猜测的 JSON 地址标成“搜狗云已接通”。

旧搜狗 Web 接口在本次检查中分别返回 404、File not found 或域名无法解析。没有从这些结果得到可工作的搜狗原生云联想客户端，也没有复制其模型文件、词库内容或反编译源码进入产品。

## 落地边界

百度预设使用合成 `nihao` 请求得到 `status: "T"`、`errno: "0"` 和 `你好` 等候选。协议形状另与 Fcitx cloudpinyin 的 [BaiduBackend](https://github.com/fcitx/fcitx5-chinese-addons/blob/master/modules/cloudpinyin/backend.cpp) 交叉检查；产品实现仍由本仓 `client-core` 独立构造和校验。默认来源与联网开关保持用户现值。

借鉴的机制是两类查询身份分离、本地结果先可用、云端异步合并、过期请求拒绝、仅点选提交预测后缀，以及关闭授权立即撤回。云预测本机授权为 `cloud_candidate_provider.prediction_enabled`，服务和凭据仍由既有云候选配置管理。后续词服务需要用户配置相应接口，不能据此声称搜狗或百度的专有预测模型已经移植。

运行 `bash scripts/test-commercial-prediction-docker.sh` 复现共享契约、引擎、运行时和 C ABI 回归。增加 `MSIME_COMMERCIAL_LIVE=1` 会在容器里对固定百度地址发送合成拼音并用生产解析器检查返回；不会读取本机输入历史、账号或私人配置。`commercial_prediction_probe -- transaction` 以同一合成词库输出旧版与新版的可观察差异，供基线、修改与回滚复核。
