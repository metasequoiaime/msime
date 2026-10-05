# 把 resources/ 下某个锁文件列出的 artifacts 下载到同一个目录。地址和 SHA-256 直接读锁文件，锁是
# 唯一的来源，这里不另抄一份哈希。单独成包，资源只在锁变化时才重新下载。
#
# 复制而不是链接：CMake 安装时会拒绝符号链接形式的资源文件（见 platforms/linux/CMakeLists.txt 里
# MSIME_ENGINE_RESOURCES 与 MSIME_HANDWRITING_MODEL_DIR 的校验），沙箱里跨 store 的硬链接也做不成。
# 支持 reflink 的文件系统上这份复制不占额外空间。
{
  lib,
  runCommand,
  fetchurl,
}:
{
  name,
  lock,
  meta ? { },
}:
runCommand name { inherit meta; } ''
  mkdir -p $out
  ${lib.concatMapStrings (artifact: ''
    cp --reflink=auto ${fetchurl { inherit (artifact) url sha256; }} $out/${artifact.name}
  '') (lib.importJSON lock).artifacts}
''
