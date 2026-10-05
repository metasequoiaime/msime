# 检查与渲染 overlay 用的 Gentoo 工具镜像，check-in-container.sh 与 ../render-definitions.sh 共用。
FROM gentoo/stage3:latest
COPY --from=gentoo/portage:latest /var/db/repos/gentoo /var/db/repos/gentoo
# 容器里用不了 Portage 的命名空间沙箱。只放行本包与 pycargoebuild 的测试关键字；全局放开会把工具链换成二进制仓库里没有的测试版本，整套从源码编译。
RUN printf '%s\n' 'FEATURES="-ipc-sandbox -mount-sandbox -network-sandbox -pid-sandbox -sandbox -usersandbox"' 'EMERGE_DEFAULT_OPTS="--getbinpkg --quiet-build --jobs=4"' >> /etc/portage/make.conf \
  && mkdir -p /etc/portage/package.accept_keywords \
  && printf '%s\n' app-i18n/msime app-portage/pycargoebuild > /etc/portage/package.accept_keywords/msime \
  && emerge --noreplace dev-util/pkgcheck app-portage/pycargoebuild dev-vcs/git dev-lang/rust-bin net-libs/nodejs \
  && rm -rf /var/cache/binpkgs/* /var/cache/distfiles/*
