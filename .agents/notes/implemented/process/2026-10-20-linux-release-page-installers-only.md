# Agent Note: Linux 版本发布页只放安装包

Status: implemented

## Problem

`linux-v0.12.1` 的版本发布页上有 42 个附件：六个版本各两个架构的 `.deb`、`.rpm`、`.tar.gz`（36 个），两个 legacy `.deb`，发行版打包用的三个源码 tarball（`msime-0.12.1.tar.xz`、`-vendor.tar.xz`、`-frontend.tar.xz`），以及一份覆盖全部附件的 `SHA256SUMS`。发布说明只有更新记录，没有一句话说明该下哪个。维护者确认过的发布页标准（`.agents/skills/release-platform/SKILL.md` 第 5 步，[macOS 的做法](2026-10-20-macos-release-page-only-dmgs.md)）是：版本发布页只放用户该下载的安装包和只列它们的 `SHA256SUMS`，发布说明开头是从 `shared/contracts/editions.json` 生成的下载指引，用户不会手动下载的文件放到各自的滚动 release。

Linux 上「该下哪个」比 macOS 难：要按发行版选格式（deb、rpm、tar.gz，还有只给旧系统的 legacy deb），再按架构选文件，而架构在 deb 里叫 `amd64`/`arm64`、在 rpm 和 tar.gz 里叫 `x86_64`/`aarch64`。

源码 tarball 不是给人下载的。它们的消费方：

- `platforms/linux/packaging/rpm/msime.spec` 的 `Source0`/`Source1`（COPR、OBS 用的 spec，`render-definitions.sh` 打 SRPM 时用本地文件，但 spec 里的地址是公开的出处）；
- `platforms/linux/packaging/debian/watch`、`debian/README.source`、`debian/copyright`、`debian/rules`（Launchpad PPA 与 OBS 的 Debian 源码包把它们作为 orig tarball）；
- `platforms/linux/packaging/gentoo/app-i18n/msime/msime.ebuild.in` 的 `SRC_URI`（前端包）和 Gentoo 的 README；
- `platforms/linux/packaging/render-definitions.sh`（`package-definitions` job 不带 `MSIME_RELEASE_DIR`，从发布页下载并核对）、`render-sources.py` 与 `obs/publish.sh` 的说明；
- `.github/workflows/publish-linux-copr.yml`、`publish-linux-obs.yml`、`publish-linux-ppa.yml`，都用 `gh release download linux-v$VERSION` 取两个 tarball。这三个可以手动触发去补发任何一个已发布的版本。

在 msime-web 里查过：下载页的 `scripts/generate-platforms.mjs` 只按文件名形状认 `.deb`、`.rpm`、`.tar.gz`，源码包和 legacy 包都不展示；镜像 `dl.msime.app`（`workers/dl/index.mjs`）放行整个 `metasequoiaime/msime/releases/download/` 前缀，不按 tag 限制；更新记录（`shared/releases.ts`）按版本号校验 tag，`linux-sources` 这样的 tag 会被丢掉。组织里的代码搜索除了这两个仓库没有别处引用这些文件名，AUR 的 `msime-bin` 取的是发布页上的 `.rpm`、`msime` 取的是标签归档，都不受影响；README 建议的 `metasequoiaime/gentoo-overlay` 仓库目前不存在。

## Decision

版本发布页 `linux-vX.Y.Z` 只放安装包：每个版本两个架构的 `.deb`、`.rpm`、`.tar.gz`，勾选了 `legacy` 时再加两个 `msime-linux-legacy_X.Y.Z_<架构>.deb`，以及只列这些安装包的 `SHA256SUMS`；发布说明开头是下载指引。源码 tarball 放到滚动的 `linux-sources` release。

### tar.gz 留在发布页

它是安装包，不是附属文件：给不经 apt 安装的 Debian 系，内容与 `.deb` 相同。官网下载页把它作为每个版本的一个下载按钮（`generate-platforms.mjs` 的 `通用压缩包 · tar.gz`），应用内更新检查在没有 `.deb` 时也取它的摘要。下载量与 `.rpm` 相当：0.11.0 的 full x86_64 tar.gz 109 次、rpm 91 次，0.12.0 是 15 次和 13 次。指引写清它只给 Debian 系、Arch 和 Fedora 上的 Fcitx5 找不到它的插件。

### legacy deb 留在发布页，指引里单独一行

它是给 glibc 2.28 到 2.34 的 Debian 系（Debian 10/11、Ubuntu 20.04、UOS 20）的安装包，这些系统装不上发布页的 `.deb`。[把它放上发布页](2026-10-10-linux-legacy-deb-release.md) 本来就是 #6311 要的：用户要一个能直接下载安装的包。挪到别处就要让这批用户去另一个 release 找，指引里多一跳却省不掉一行。指引在格式说明里写明它适用的系统、只有完整版的 IBus 输入法、没有设置窗口，表格最后一行给两个架构的链接；这次没有构建它（取消勾选 `legacy`）时这些内容都不出现。

### 下载指引

`platforms/linux/scripts/edition_linux.py download-guide --version V --dist DIR --download-base URL [--repositories]` 生成，夹在 `<!-- msime-download-guide -->` 与 `<!-- /msime-download-guide -->` 之间，与 macOS 相同。先按系统选格式（deb、rpm、legacy deb、tar.gz、软件源），再按 `uname -m` 选架构列，然后是每个版本一行的表，完整版排第一、标为推荐。版本名、包名和 `<包名>-setup` 取自版本表；每个链接的文件名取自这次 `dist/` 里实际要发布的文件，按 `release-linux.yml` 核对的同样的形状匹配，某个版本某个架构缺一种格式就失败，不会链接到不存在的文件。软件源一行（COPR、PPA、OBS、AUR，指向 README「包管理器」）只在 main 的非预发布里出现，因为软件源只在正式版之后更新。

publish job 拆成三步：先在本地备齐 `SHA256SUMS`、更新记录和指引（任何一步失败都还没上传任何东西），再上传源码包，最后发布版本页。

### 源码包在 linux-sources

- `make-source-tarballs.sh` 另写一份只列三个 tarball 的 `msime-VERSION-SHA256SUMS`。`linux-sources` 上放着每个版本的 tarball，所以校验文件按版本命名。
- `packaging/publish-sources.sh` 在 linux-sources 不存在时创建它（预发布、`--latest=false`），先对着 `msime-VERSION-SHA256SUMS` 核对、确认它只列这三个文件，再上传，并用 GitHub 为资产记录的 `digest` 逐个比对。每个版本一组、不清理：已发布版本的 spec、ebuild 按地址引用它们。
- 只有从 main 发出的发布上传。`package-definitions`、COPR、OBS、PPA 也只对 main，beta 与 alpha 的源码包从来没有消费方；而且 tarball 名不带 `-beta` 后缀，传上去会覆盖同一版本号的正式版。它们只留在 workflow artifact 里。
- 上传在版本发布页之前：发布页上线时，包定义要的源码包已经在 linux-sources 上。同一版本重跑时 `--clobber` 覆盖，与 `publish-release.sh` 覆盖版本页一致；`make-source-tarballs.sh` 对同一提交生成相同的字节，从同一提交重跑不改变任何地址下的内容。
- 消费方：spec 的 `Source0`/`Source1`、`debian/watch`（改读 `releases/tags/linux-sources` 这一个 release 的 JSON，不读会随别的发布增多而翻页的发布列表）、`debian/README.source` 与 `copyright`、Gentoo 的 `SRC_URI` 全部改到 `releases/download/linux-sources/`。`render-definitions.sh` 从 linux-sources 取 tarball，必须列在 `msime-VERSION-SHA256SUMS` 里并对上，Gentoo 的前端包也先核对再放进 distfiles（以前直接由 Portage 下载、只在 Manifest 里记下现算的摘要）；版本页的 `SHA256SUMS` 只在渲染 AUR 时下载。
- 三个 publish-linux workflow 改用 `packaging/download-sources.sh`：linux-sources 上有这个版本就从那里取，否则从它自己的 `linux-vX.Y.Z` 发布页取，这样手动补发 0.12.1 及更早的版本照常工作。脚本取自触发运行的提交（`tools/` 的 sparse checkout），不取自旧标签。DIR 里的校验文件保留原名：旧标签自己检出里的 `render-definitions.sh` 认 `SHA256SUMS`，新的认 `msime-VERSION-SHA256SUMS`。

### 已发布的 linux-v0.12.1

不改。已发布的 release 不在这次的授权范围内，也有真实的依赖：0.12.1 标签里的 spec 和 debian/watch 指向 `linux-v0.12.1` 下的 tarball。要迁移时的步骤见 PR 说明，写在这里的只是要点：把三个 tarball 原样复制到 linux-sources 并补一份 `msime-0.12.1-SHA256SUMS`、按摘要核对；把版本页的 `SHA256SUMS` 换成只列安装包的版本（安装包的行逐字不变，AUR `msime-bin` 读它）；再删三个 tarball；改写发布说明加上指引要先问维护者。已经发出去的 0.12.1 Gentoo ebuild（如果有人发布过）的 `SRC_URI` 会因此 404，要同时改；COPR、OBS、Launchpad 都已持有上传时的 tarball，不再下载。

## Alternatives considered

- **源码包留在版本发布页，只靠指引说明哪些不用下** — 最强的理由是不碰任何消费方，零风险。否定：维护者的标准就是发布页只放安装包；三个 `.tar.xz` 在按名字排序的附件列表里夹在安装包中间，指引写得再清楚，附件列表本身仍是一长串。
- **每个版本一个源码 release（`linux-sources-v0.12.2`）** — 最强的理由是与版本一一对应，地址里带版本 tag，删一个版本也只删一个 release。否定：Releases 列表里每次发版多一个条目；debian/watch 要扫描发布列表才能找到最新的那个，而发布列表按时间分页，正是现在这份 watch 的毛病；一个滚动 release 里文件名本来就带版本号，不会互相覆盖。
- **源码包放到官网镜像 dl.msime.app** — 最强的理由是国内下载快。否定：镜像在另一个仓库，这个 workflow 没法在同一次运行里原子地上传并核对；镜像从不重新校验缓存，只适合内容不变的地址。构建农场（COPR、OBS、Launchpad）都不在国内，也用不上它的速度。
- **tar.gz 也搬走或不再发布** — 最强的理由是它只服务不经 apt 的 Debian 系，用户面窄，去掉后每个版本少 12 个附件。否定：它是安装包，官网下载页给每个版本提供它，下载量与 rpm 相当；去掉要同时改官网下载页和应用内更新的回退逻辑，换来的只是附件少一些。
- **legacy deb 搬到 linux-sources 或单独的 release** — 最强的理由是它只给少数旧系统（0.12.1 两个架构共 6 次下载），放在主列表里让多数人多看一行。否定：它是这批用户唯一能装的安装包，#6311 要的就是在发布页上直接拿到；搬走后指引仍要为它写一行，只是多了一跳。

## Consequences

- **收益**：每个版本的发布页从 42 个附件减到 39 个（不勾选 legacy 时 37 个），`SHA256SUMS` 只列安装包；发布说明第一段按系统、架构、版本告诉人该下哪个，链接一定指向这次真实发布的文件。源码包多了两道过去没有的核对：上传后按 GitHub 摘要比对；`render-definitions.sh` 渲染 Gentoo 时也核对前端包。
- **代价与已知上限**：
  - `linux-sources` 作为预发布出现在 Releases 列表里，而且随版本只增不减（每个版本约 120 MB）。每个版本 4 个资产都放在同一个 release 上；哪天这个 release 的资产多到不便管理或碰到 GitHub 的限制，再按版本区间拆成几个。
  - beta 与 alpha 的源码包不再挂在任何 release 上，只在 workflow artifact 里，90 天后过期。它们之前挂在预发布页上，但没有任何消费方。
  - 0.12.1 及更早版本的源码包仍在各自的版本发布页上，`download-sources.sh` 的回退分支要一直保留到这些版本都迁移或不再需要补发。
  - 指引里的系统列表（Debian 12、Ubuntu 22.04、Fedora 44、glibc 2.28–2.34）是手写的，与 `platforms/linux/README.md`「生成 Linux 安装包」「旧发行版的 legacy 包」两节相同；构建基线变了（例如换成 Debian 13 容器）要一起改。
  - 官网下载页的更新记录把发布说明逐行显示；msime-web#458 已经让它按那对注释跳过下载指引，Linux 用同一对注释，不用再改。

## Verification

- `python3 scripts/test-linux-release-page.py`（`gh`、`curl`、`docker` 是临时目录里的替身）：指引为六个版本、两个架构各链接 deb、rpm、tar.gz，完整版排第一；没有 legacy 包时不提 legacy，只有一个架构的 legacy 包、缺任何一个安装包都失败；预发布不指向软件源。`publish-sources.sh` 创建预发布、不设 latest 的 linux-sources，上传四个文件、按摘要核对，第二个版本不动第一个版本的文件；校验对不上或校验文件多列了文件时一次 `gh` 都不调用。`download-sources.sh` 从 linux-sources 取新版本，0.12.1 退回它自己的发布页，篡改过的 tarball 和哪里都没有的版本都失败。`render-definitions.sh` 从 linux-sources 下载并核对，tarball 对不上时不起 rpmbuild 容器。`release-linux.yml` 只把安装包和 `SHA256SUMS` 交给 `publish-release.sh`，三个步骤的顺序是备齐、上传源码包、发布；spec、watch、ebuild 都指向 linux-sources，packaging 与 Linux workflow 里没有剩下 `releases/download/linux-v…/….tar.xz`。
- 用 `linux-v0.12.1` 实际的 38 个安装包文件名跑 `download-guide`：38 个全部链接到，没有多出的链接。
- `actionlint` 检查了改过的四个 workflow；`shellcheck` 检查了 packaging 下的四个脚本；`scripts/test-linux-editions.py`、`test-linux-distro-packaging.py`、`test-editions.py` 照常通过。
- 没有在分支上触发 Release Linux：`publish` 关掉时 publish job 整个跳过，改动的三步一步都不会跑，而 deb、rpm、legacy 构建没有改动，跑一遍只验证了 `make-source-tarballs.sh` 多写的那一个文件。第一次真正的验证是下一个 Linux 正式版。
