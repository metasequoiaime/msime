# desktop-dictionary.lock.json 钉住的词库。地址和 SHA-256 直接读锁文件，锁是唯一的来源，
# 这里不另抄一份哈希。单独成包，代码改动不会让 180 MB 的词库再复制一遍。
{
  lib,
  runCommand,
  fetchurl,
}:
let
  lock = lib.importJSON ../../../resources/desktop-dictionary.lock.json;
in
# 复制而不是链接：CMake 安装时会拒绝符号链接形式的词库文件（见 platforms/linux/CMakeLists.txt
# 里 MSIME_ENGINE_RESOURCES 的校验），沙箱里跨 store 的硬链接也做不成。支持 reflink 的文件系统
# 上这份复制不占额外空间。
runCommand "msime-resources" { } ''
  mkdir -p $out
  ${lib.concatMapStrings (artifact: ''
    cp --reflink=auto ${fetchurl { inherit (artifact) url sha256; }} $out/${artifact.name}
  '') lock.artifacts}
''
