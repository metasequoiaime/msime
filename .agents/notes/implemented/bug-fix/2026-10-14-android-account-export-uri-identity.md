# Agent Note: Android 账号导出的分享路径保持独立

Status: implemented

## Problem

`ProfilePage.export()` 按日期给云端数据归档命名。再次导出时先删掉整个 `exports` 缓存目录里的旧文件，再把新归档写到同一路径。外部应用拿到的 `FileProvider` URI 是路径引用，可能延后读取；如果用户切换账号并再次导出，旧 URI 就会读到新账号的数据。

## Decision

每次导出在日期后附加独立 UUID，给每个分享 URI 固定的归档路径。新导出保留一天内的旧归档供已打开的分享目标读取，只清理超过一天的本应用归档。失败和账号代次失效时只删除本次导出的文件。公开的 `exportFileName(date)` 仍表示供人识别的日期文件名，私有缓存路径使用 `exportCacheFileName(date, id)`。

## Alternatives considered

- 继续只保留最新归档、仅改文件名：能够避免旧 URI 读到新账号内容，但分享目标稍晚读取时旧文件已经消失，导出操作会失败。
- 永久保留全部导出：已分享 URI 不会失效，但包含个人数据的缓存会无界增长，因此在下次导出时清理超过一天的旧归档。

## Verification

`DeviceDataApiSmoke` 的同日两次导出独立路径断言在实现前因缺少接口而失败；实现后 `bash platforms/android/check-host.sh` 通过，运行 230 个 Android JVM smoke 并编译 91 个设备套件源码。

## Consequences

接收方延后读取仍会拿到原归档；缓存可能同时保留一天内的多份个人数据，下一次导出时会清理过期文件，系统也可清理应用缓存。
