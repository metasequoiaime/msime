# 隔离遥测验收

正式产品继续固定使用生产服务。合成故障只向专用的本机回环或转发至隔离后台的回环发送，不用于生产服务。

复用已有构建缓存，在编译前固定端口并显式开启测试特性：

```sh
MSIME_TEST_TELEMETRY_ORIGIN=http://127.0.0.1:48153 cargo build -p msime-host-api --features telemetry-loopback
python3 tools/crash/verify_telemetry_loopback.py --library target/debug/libmsime_host_api.dylib --origin http://127.0.0.1:48153
```

Linux 换成 `.so`，Windows 换成对应的 `.dll` 路径。桌面外壳通过 `msime-desktop/telemetry-loopback` 开启，两条实际发送入口共用同一个工厂；MCP 仅采集，由原生宿主补报。设备需显式转发此回环端口，脚本本身不安装、不启动产品。

测试目标仅接受无尾斜线的 `http://127.0.0.1:<非零规范端口>`。配置在编译时读取，运行时环境不能修改；缺失或不合法时发送器构造失败，队列保留，不会回退生产。账号、公告等非遥测服务仍保持原地址，验收不得调用这些服务。

`verify_telemetry_loopback.py` 每条请求带测试专用的 `test_origin` 握手，当前正式库因未知字段而拒绝，测试库则核对编译目标。工具先用两个不联网的 `begin` 请求核对目标：错误目标也成功的旧库会在任何 `flush` 前被拒绝，避免旧库忽略未知字段后误发。脚本使用合成本地记录验证 C ABI、HTTP 收件、升级补报仍保留原版本、503 后重试保留事件身份，以及关闭后不发送；它不替代实际故障捕获、产品重启、真实后端或符号化验收。

对未启用特性的正式库、或未配置目标的测试库，用相同命令追加 `--expect-rejected`，核对开始与发送均在联网前拒绝。运行其他产品测试前必须重建不带特性的正式库，禁止把测试产物用于安装或发布。
