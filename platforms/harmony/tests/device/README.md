# HarmonyOS 候选画布设备验收

手写回归可直接检查已安装的产品包：在水杉应用内打开「试用键盘」的空白输入框并切到手写，保持设备解锁，执行 `python3 platforms/harmony/tests/device/check-handwriting.py --hdc /path/to/hdc --serial <设备>`。脚本注入合成「工」的三笔，检查候选、选字上屏、空画布删除与重写清空，不发送消息、不读取其他应用。它使用设备的 `uitest`，运行期间不要并行操作同一设备。

需要 DevEco 的 `hvigorw` / `ohpm`、已经按平台 README 暂存的设置页、引擎资源、原生库及语音 HAR。在本工作区没有其他构建运行时执行：

```sh
python3 platforms/harmony/tests/device/build-canvas-probe.py
python3 platforms/harmony/tests/device/check-canvas.py \
  --hdc /path/to/hdc --serial 127.0.0.1:15334 \
  --hap target/harmony-tests/device/canvas-probe.hap
```

设备必须是专用模拟器。Python 需要 Pillow。构建器临时修改本工作区的应用 ID、编辑器入口和扩展初始化，复制测试模板与合成图片，打包后在 `finally` 恢复所有产品源文件。不要和该工作区的其他构建并行运行；测试模板不会编进常规 HAP。

测试包名为 `app.msime.hmos.canvasprobe`，不覆盖已安装的 `app.msime.hmos`，不声明原应用的共享沙箱。扩展先通过产品偏好 API 保存合成配置，再走原来的会话、原生键盘与候选绘制路径。编辑器是原生 `TextArea`，不替换输入法框架或候选组件。初始合成面板在红/蓝之间变化，挂件在绿/黄之间变化；前两阶段截图必须实际出现这四种颜色，而且每种面板色的像素数须超过屏宽 × 20，确保九宫格填满了候选条内部，坏图阶段只能出现挂件的两种颜色，随后点击候选检查 `你好` 上屏，并验证隐藏后重新显示。原输入法在退出时恢复，独立测试包随之卸载。证据只包含合成编辑器，写入 `target/harmony-tests/device/evidence/`。

初始化模板每次扩展进程重建交替使用浅/深色，第 3/4 次使用压缩数据损坏但 PNG 结构及 CRC 正确的面板，自动脚本按四个阶段依次验收浅色、深色、浅色坏图和深色坏图；调试时可用 `--phases 1` 只跑首次浅色。只有实际通过的阶段才计入验收。偏移、SOURCE/OVER、三种 disposal、CRC、序号及预算由 `tests/candidate-apng.test.ts` 的纯逻辑测试覆盖。物理设备、系统减弱动态效果和所有 Windows 上游皮肤的截图对比仍需单独验证。
