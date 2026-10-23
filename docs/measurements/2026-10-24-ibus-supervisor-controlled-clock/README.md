# IBus 监督器测试的受控时钟验证

全拼路径页压缩的普通 push 门禁顺带暴露两次既有检查失败。本目录保留失败及复测记录，不覆盖[路径页测量](../2026-10-23-quanpin-path-page-compaction/README.md)原本通过的完整 quick 证据。

第一次是容器 I/O 极慢时 `fcitx5-candidate-theme-hint` 在 1503.02 秒后超时；同一二进制单项复测在 937.73 秒通过，第二次完整门禁中又在 22.64 秒通过。第二次是监督器测试将宿主调度延迟算进退避上界：2、4、8 秒请求对应的启动间隔为 2.052、4.060、10.636 秒。

监督器回归让最后一个宿主故意迟到 2 秒，旧断言失败（第三个间隔 10.038 秒）。修正后的夹具使用私有 `PATH` 内的 `date` 与 `sleep`，精确记录退避请求及对应宿主次数；停止等待场景阻塞到收到停止信号，按就绪状态触发请求。完整脚本通过，错误退避、升级等待与不终止等待三个变体均被检出。生产启动器不变。

修正后完整本地 quick 在 1738.943 秒后退出 0。Linux 原生套件注册 111 项，110 项通过，按需激活因缺少真实 daemon 依赖沿用原规则跳过；监督器用时 4.18 秒，主题提示用时 8.22 秒。

## 复现

在构建 Linux 原生门禁后，从仓库根运行单项测试：

```sh
task_gate_image=msime-linux-build-gate:622a8a3046ad
docker run --rm --init -v "$PWD:/source" -v "$PWD/target/linux-build-gate:/build" -w /source "$task_gate_image" ctest --test-dir /build/cmake --output-on-failure -R '^linux-ibus-launcher-supervisor$'
docker run --rm --init -v "$PWD:/source" -w /source "$task_gate_image" python3 docs/measurements/2026-10-24-ibus-supervisor-controlled-clock/launcher-mutations.py
```

本次镜像为 `msime-linux-build-gate:622a8a3046ad`；其他版本以 `platforms/linux/build-container.sh` 的输出为准。完整门禁仍运行 `bash scripts/verify-local.sh --quick`，没有增加失败基线或调整 CTest、子进程超时。

## 文件

`push-attempt-1*`、`theme-hint-rerun*`、`push-attempt-2*` 保留两次 push 门禁的失败和第一项单独复测。`launcher-jitter-red*`、`launcher-jitter-green*`、`launcher-mutations*` 保留监督器回归的红、绿与错误变体验证。`quick-after-launcher*` 记录夹具修正后的完整本地门禁。`package.py` 清除本机路径、固定 gzip 时间戳并生成逐文件 SHA-256；`manifest.json` 不把自己计入哈希。

## 覆盖限制

夹具验证退避决策和进程生命周期，不测操作系统 sleep 精度，也不覆盖健康运行 30 秒后的退避重置。编译和单测不能代表真实 IBus、Android 设备或其他平台装机验收。默认九键 16 ms 仍未验证。
