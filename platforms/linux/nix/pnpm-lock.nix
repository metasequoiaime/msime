# 按 pnpm-lock.yaml 里每个包的 integrity 逐个下载，再把锁文件改写成从 store 里的 tarball 安装：
# pnpm 拿改写后的锁文件就能 --offline --frozen-lockfile 装好。锁文件就是 pin，依赖变了不用另改哈希。
#
# 只认 lockfile v9 的 packages: 段里 registry 包的写法（resolution 只有 integrity），逐行匹配，
# 不是通用的 YAML 解析；遇到别的写法（git、tarball 地址等）求值时直接失败，而不是装出缺包的结果。
# 带 os、cpu、libc 限制的包只取本机能用的，其余不下载，pnpm 本来也会跳过它们。
{
  lib,
  stdenv,
  fetchurl,
  writeText,
}:
{
  lockfile,
  registry ? "https://registry.npmjs.org",
}:
let
  text = builtins.readFile lockfile;
  lines = lib.splitString "\n" text;

  unquote = value: lib.removePrefix "'" (lib.removeSuffix "'" value);

  # 逐行收集 packages: 段里每个包的 integrity 与平台限制。段的边界是顶层键（行首不缩进）。
  parsed =
    lib.foldl'
      (
        state: line:
        let
          key = builtins.match "  '?([^' ]+)'?:" line;
          resolution = builtins.match "    resolution: \\{integrity: ([^,}]+)}" line;
          constraint = builtins.match "    (os|cpu|libc): \\[(.*)]" line;
          set =
            field: value:
            state
            // {
              packages = state.packages // {
                ${state.current} = state.packages.${state.current} // {
                  ${field} = value;
                };
              };
            };
        in
        if builtins.match "[^ ].*" line != null then
          state // { inPackages = line == "packages:"; }
        else if !state.inPackages then
          state
        else if key != null then
          state
          // {
            current = builtins.head key;
            packages = state.packages // {
              ${builtins.head key} = { };
            };
          }
        else if resolution != null then
          set "integrity" (builtins.head resolution)
        else if lib.hasPrefix "    resolution:" line then
          throw "pnpm-lock.nix: ${state.current} 不是只带 integrity 的 registry 包：${line}"
        else if constraint != null then
          set (builtins.elemAt constraint 0) (
            map unquote (lib.splitString ", " (builtins.elemAt constraint 1))
          )
        else
          state
      )
      {
        inPackages = false;
        current = null;
        packages = { };
      }
      lines;

  # npm 的平台字段：全是 "!xxx" 时是排除列表，否则是允许列表。
  host = {
    os = stdenv.hostPlatform.node.platform;
    cpu = stdenv.hostPlatform.node.arch;
    libc = if stdenv.hostPlatform.isMusl then "musl" else "glibc";
  };
  allows =
    package: field:
    let
      list = package.${field} or [ ];
    in
    list == [ ]
    || (
      if lib.all (lib.hasPrefix "!") list then
        !(lib.elem "!${host.${field}}" list)
      else
        lib.elem host.${field} list
    );
  usable = package: lib.all (allows package) (builtins.attrNames host);

  tarball =
    key: package:
    let
      parts = builtins.match "(@?[^@]+)@(.+)" key;
      name = builtins.elemAt parts 0;
      version = builtins.elemAt parts 1;
    in
    assert lib.assertMsg (package ? integrity) "pnpm-lock.nix: ${key} 没有 resolution";
    fetchurl {
      url = "${registry}/${name}/-/${lib.last (lib.splitString "/" name)}-${version}.tgz";
      hash = package.integrity;
    };

  # 同一份 tarball 的 integrity 相同，按 integrity 替换不会张冠李戴。
  selected = lib.filterAttrs (_: usable) parsed.packages;
  resolutions = lib.mapAttrsToList (key: package: {
    from = "resolution: {integrity: ${package.integrity}}";
    to = "resolution: {integrity: ${package.integrity}, tarball: 'file:${tarball key package}'}";
  }) selected;
in
assert lib.assertMsg (parsed.packages != { }) "pnpm-lock.nix: ${toString lockfile} 里没有找到 packages:";
writeText "pnpm-lock.yaml" (
  builtins.replaceStrings (map (r: r.from) resolutions) (map (r: r.to) resolutions) text
)
