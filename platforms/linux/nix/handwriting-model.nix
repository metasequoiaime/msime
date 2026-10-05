# handwriting-model.lock.json 钉住的 Zinnia 手写模型和它的 LGPL-2.1 许可证，`msime-linux-handwriting
# --local` 用它离线识别。与 resources.nix 一样直接读锁文件的地址和 SHA-256，不另抄一份哈希；
# 单独成包，代码改动不会让 26 MB 的模型再复制一遍。
{
  lib,
  runCommand,
  fetchurl,
}:
let
  lock = lib.importJSON ../../../resources/handwriting-model.lock.json;
in
# 复制而不是链接：CMake 安装时会拒绝符号链接形式的模型文件（见 platforms/linux/CMakeLists.txt
# 里 MSIME_HANDWRITING_MODEL_DIR 的校验）。
runCommand "msime-handwriting-model"
  {
    # 与 debian/copyright 对模型的记载一致，许可证全文随包装在 HandwritingModel-LICENSE.txt。
    meta.license = lib.licenses.lgpl21Only;
  }
  ''
    mkdir -p $out
    ${lib.concatMapStrings (artifact: ''
      cp --reflink=auto ${fetchurl { inherit (artifact) url sha256; }} $out/${artifact.name}
    '') lock.artifacts}
  ''
