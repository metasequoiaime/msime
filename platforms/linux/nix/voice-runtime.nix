# voice-runtime.lock.json 为本机平台钉住的 sherpa-onnx 运行库，`msime-voice-local` 用 dlopen 打开它做
# 本地语音识别。与其他平台一样用上游的预编译库，不自己编 onnxruntime（见 scripts/fetch_voice_runtime.py）；
# 地址和 SHA-256 直接读锁文件。
{
  lib,
  stdenv,
  fetchurl,
}:
let
  lock = lib.importJSON ../../../resources/voice-runtime.lock.json;
  # 锁里 Linux 条目的名字是 linux-<uname -m>，对应 Nix 的 <uname -m>-linux。锁是支持哪些架构的唯一
  # 来源：meta.platforms 由它推出，default.nix 据此在没有钉住运行库的架构上不带运行库。
  linuxArchitectures = map (lib.removePrefix "linux-") (
    lib.filter (lib.hasPrefix "linux-") (lib.attrNames lock.platforms)
  );
  artifact = lock.platforms."linux-${stdenv.hostPlatform.uname.processor}";
in
stdenv.mkDerivation {
  pname = "msime-voice-runtime";
  inherit (lock) version;

  src = fetchurl { inherit (artifact) url sha256; };
  # 锁里的 libraries 是相对归档根目录的路径。
  sourceRoot = ".";
  # 保持上游二进制原样，只补 RUNPATH。
  dontStrip = true;

  # 保留上游的 $ORIGIN（C API 靠它找到旁边的 libonnxruntime.so），只在后面追加编译器运行库：NixOS 上
  # 没有全局的 libstdc++。不用 autoPatchelfHook：它把 $ORIGIN 换成本包的绝对路径，装进插件包的那份就会
  # 转去加载这里的 libonnxruntime.so。
  #
  # 上游归档只有库，不带许可证。本包也会被单独分发，所以装上仓库里固定的那几份，与插件包里 CMake 装的
  # 同源同名（见 platforms/linux/CMakeLists.txt 的 MSIME_NOTICE_SOURCES）；放在子目录里，CMake 只按库名
  # 从顶层取库，不受影响。
  installPhase = ''
    runHook preInstall
    install -Dm755 -t $out ${lib.escapeShellArgs artifact.libraries}
    patchelf --add-rpath ${lib.getLib stdenv.cc.cc}/lib $out/*.so
    doc=$out/share/doc/msime-voice-runtime
    install -Dm644 ${../../../shared/voice/third_party/sherpa-onnx/LICENSE} $doc/sherpa-onnx-Apache-2.0.txt
    install -Dm644 ${../data/licenses/onnxruntime-MIT.txt} $doc/onnxruntime-MIT.txt
    install -Dm644 ${../data/licenses/onnxruntime-ThirdPartyNotices.txt} $doc/onnxruntime-ThirdPartyNotices.txt
    runHook postInstall
  '';

  meta = {
    description = "水杉输入法本地语音识别用的 sherpa-onnx 运行库（上游预编译）";
    homepage = lock.source;
    # sherpa-onnx 是 Apache-2.0，随附的 ONNX Runtime 是 MIT。
    license = [
      lib.licenses.asl20
      lib.licenses.mit
    ];
    sourceProvenance = [ lib.sourceTypes.binaryNativeCode ];
    platforms = map (architecture: "${architecture}-linux") linuxArchitectures;
  };
}
