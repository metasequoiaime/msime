# 动态图片解码器来源

`include/msimeui/AnimatedImage.h`、`src/AnimatedImage.cpp` 及 `../tests/ui/candidate_image_animation.cpp` 中的 APNG/GIF 与时钟测试移植自 [msime-windows](https://github.com/metasequoiaime/msime-windows/tree/dd018f79858314ed542a47c528608752b13bd261/ui)，固定提交 `dd018f79858314ed542a47c528608752b13bd261`，源路径分别为 `ui/include/msimeui/AnimatedImage.h`、`ui/src/AnimatedImage.cpp`、`ui/tests/src/test_animated_image.cpp`。来源只读取该提交的 Git 对象，不复制邻近工作区的未提交内容。

来源 `ui/LICENSE` 的 GPL-3.0 全文保存在本目录 [LICENSE](LICENSE)，与本项目 GPL-3.0-only 一起分发，不引入额外分发限制。Windows 安装包的 `Collect-Notices.ps1` 同时收集本文件和本目录许可证，项目许可证仍按原流程附带，不增加独立二进制或系统编解码器。

本地调整：注释使用中文；默认限制收紧到 120 帧、2048 边长、8MiB 输入和累计八百万像素；静态回退也检查解码预算，失败的动画快照在静态回退前清理；流长度转换同时兼容 32／64 位，保留截断检查；候选窗自己的 `CandidateImageFrames` 共享像素预算，只上传当前 GPU 帧，并管理帧边界定时器、系统动画偏好、隐藏与设备恢复。原有材质合成器未被上游文件覆盖。WIC 的 WebP 多帧能力仍取决于系统编解码器，与来源相同；此通路不承诺修正解码器未提供的 WebP 帧时长。
