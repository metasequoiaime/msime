# Nix 沙箱不联网：在固定输出下载阶段按同一份 OpenCC 锁准备输入。
{
  lib,
  fetchurl,
  runCommand,
}:
let
  lock = builtins.fromJSON (builtins.readFile ../../../resources/opencc.lock.json);
  inputs = map (entry: {
    name = builtins.baseNameOf entry.path;
    source = fetchurl {
      url = "https://raw.githubusercontent.com/metasequoiaime/msime-dictionary/${lock.commit}/${entry.path}";
      inherit (entry) sha256;
    };
  }) lock.files;
in
runCommand "msime-opencc-inputs" { } ''
  mkdir -p "$out"
  ${lib.concatMapStringsSep "\n" (entry: ''ln -s ${entry.source} "$out/${entry.name}"'') inputs}
''
