# Server manifest link probe

The production Server includes `ServerManifest.cmake`. This minimal target uses the same wiring without starting the Server or registering a TSF component, so a manifest change can be checked without a full platform build.

`MSIME_SERVER_UIACCESS` 默认关闭。`Build-Client.ps1 -ServerUiAccess` 仅供随后可信签名的安装构建开启，其他构建显式传 OFF，覆盖旧 CMake 缓存。UIAccess 清单保持 `asInvoker`、`uiAccess="true"`，需要受信任签名和安全安装目录；未签名和 SignPath 测试证书包使用普通权限。

在 Windows 上运行 `pwsh -File platforms/windows/tests/server-manifest/verify.ps1`：同一构建目录依次 OFF → ON → OFF，直接读生成 PE 的资源 1，核对实际清单，并实际启动两次未签名普通权限程序。CI 的 Windows 脚本作业运行同一检查。安装冒烟另以 `MetasequoiaImeServer.exe --help` 核对完整产品的加载与启动。

Local cross-link verification, run from the repository root:

```sh
cmake -S platforms/windows/tests/server-manifest -B /tmp/msime-manifest-probe \
  -DCMAKE_SYSTEM_NAME=Windows \
  -DCMAKE_CXX_COMPILER=x86_64-w64-mingw32-g++ \
  -DCMAKE_RC_COMPILER=x86_64-w64-mingw32-windres \
  -DMSIME_SERVER_UIACCESS=ON
cmake --build /tmp/msime-manifest-probe
x86_64-w64-mingw32-windres -J coff -O rc /tmp/msime-manifest-probe/msime-client-server.exe
```

The extracted PE must contain RT_MANIFEST (24), resource ID 1, and `requestedExecutionLevel` with `asInvoker` and `uiAccess="true"`. A native configure and build with the option OFF must work without an RC compiler. MSVC disables linker-generated manifests because RC already embeds resource 1. This probe checks the embedded resource; the signed Server it stands in for is built by `../../Build-Client.ps1` and signed and installed through `../../installer/Package-SimplySign.ps1`.
