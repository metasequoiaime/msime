# 世界时钟城市目录

`zone.tab` 与 `LICENSE` 原样来自 IANA 公共领域数据归档：

- 来源：https://data.iana.org/time-zones/releases/tzdata2026a.tar.gz
- 归档 SHA-256：`77b541725937bb53bd92bd484c0b43bec8545e2d3431ee01f04ef8f2203ba2b7`
- 上游固定版本：2026a；分发许可为公共领域，见 `LICENSE`，无额外分发限制。
- 城市名称由宿主的离线 ICU 本地化；没有本地化时显示 IANA 城市名。
- 北京补充坐标为北纬 39.9042、东经 116.4074，时区 `Asia/Shanghai`；它是本项目编写的常用城市条目。

时区规则由 Jiff 的 `TimeZoneDatabase::bundled()` 内嵌（`jiff-tzdb` 0.1.8，2026c），太阳事件由 `sunrise` 计算。依赖版本和校验在工作区 `Cargo.lock`；这些 crate 的原始许可证汇总在 `resources/licenses/world-clock-NOTICE.txt`，各宿主随包分发。时钟不读取定位、不联网；系统卡片的太阳时间明确标注参考城市。
