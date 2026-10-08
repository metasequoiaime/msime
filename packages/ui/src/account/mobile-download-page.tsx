import { useId, useState } from "react";
import { FluentIcon, type FluentIconName } from "../core/fluent-icons";
import { useToast } from "../core/toast";
import { useMountedRef } from "../settings/use-mounted-ref";
import * as account from "./account-style";

/**
 * HarmonyOS 手机上 我的 内推入页面的页头：起始边是用于返回的 '‹ 我的'，页面标题在 44px 的栏里居中。
 */
export function MeSubpageHeader({
  title,
  onBack,
  backDisabled,
}: {
  title: string;
  onBack: () => void;
  backDisabled?: boolean;
}) {
  return (
    <div className={account.subpageHeader}>
      <button
        type="button"
        className={account.subpageBack}
        aria-label="返回我的"
        disabled={backDisabled}
        onClick={onBack}
      >
        <svg
          width="12"
          height="21"
          viewBox="0 0 12 21"
          fill="none"
          stroke="currentColor"
          strokeWidth="2.4"
          strokeLinecap="round"
          strokeLinejoin="round"
          aria-hidden="true"
        >
          <path d="M10 2 2 10.5 10 19" />
        </svg>
        <span aria-hidden="true">我的</span>
      </button>
      <h2 className={account.subpageTitle}>{title}</h2>
    </div>
  );
}

type DownloadPlatform = {
  name: string;
  icon: FluentIconName;
  /** 获取安装包的方式，措辞沿用 Android 的 `DownloadPage`；不写版本号，否则会过时。 */
  meta: string;
  /** 本设备，无需下载。 */
  current?: boolean;
};

// 这些分发信息与 `platforms/android/.../home/DownloadPage.java` 保持一致；两边要一起改。
const desktopPlatforms: readonly DownloadPlatform[] = [
  { name: "HarmonyOS 2in1", icon: "laptop", meta: "电脑与平板二合一 · 从源码构建" },
  { name: "Windows", icon: "window", meta: "TSF 输入法 · GitHub 发布页下载" },
  { name: "macOS", icon: "laptop", meta: "InputMethodKit · 自带自动更新" },
  { name: "Linux", icon: "desktop", meta: "IBus 与 Fcitx5 · DEB、RPM 与 TGZ" },
];

const mobilePlatforms: readonly DownloadPlatform[] = [
  { name: "iOS", icon: "phone", meta: "TestFlight 测试版" },
  { name: "iPadOS", icon: "tablet", meta: "与 iPhone 共用同一个 TestFlight" },
  { name: "Android", icon: "phone", meta: "各版本的 APK 在 GitHub 发布页" },
  { name: "HarmonyOS", icon: "phone", meta: "从源码构建", current: true },
];

/** 其他平台下载 这一行提供的平台数：这里列出的除本设备以外的所有平台。 */
export const otherPlatformCount = [...desktopPlatforms, ...mobilePlatforms].filter(
  (item) => !item.current,
).length;

/** 头图上显示的地址：去掉协议和末尾斜杠。 */
function displayUrl(url: string): string {
  return url.replace(/^https?:\/\//, "").replace(/\/$/, "");
}

function PlatformGroup({
  title,
  platforms,
  onOpen,
}: {
  title: string;
  platforms: readonly DownloadPlatform[];
  onOpen?: () => void;
}) {
  const titleId = useId();
  return (
    <section className={account.meGroup} aria-labelledby={titleId}>
      <h3 id={titleId} className={account.meGroupTitle}>
        {title}
      </h3>
      <div className={account.downloadRows}>
        {platforms.map((item) => {
          const content = (
            <>
              <span className={account.downloadTile} aria-hidden="true">
                <FluentIcon name={item.icon} size={20} />
              </span>
              <span className={account.downloadRowText}>
                <span className={account.downloadRowName}>{item.name}</span>
                <span className={account.downloadRowMeta}>{item.meta}</span>
              </span>
              {(item.current || onOpen) && (
                <span className={account.downloadRowPill(Boolean(item.current))}>
                  {item.current ? "当前设备" : "获取"}
                </span>
              )}
            </>
          );
          return item.current || !onOpen ? (
            <div key={item.name} className={account.downloadRow}>
              {content}
            </div>
          ) : (
            <button
              key={item.name}
              type="button"
              className={`${account.downloadRow} ${account.downloadRowButton}`}
              aria-label={`获取，${item.name}`}
              onClick={onOpen}
            >
              {content}
            </button>
          );
        })}
      </div>
    </section>
  );
}

/**
 * HarmonyOS 手机上的 我的 → 其他平台下载。所有平台只有一个真正的下载页面，所以头图复制它的地址供在电脑上打开，其他每个平台的行也都打开同一个页面；设计稿的 发送链接 需要本宿主没有的邮件接口，各平台的版本号也会过时，所以两者都不画。
 */
export function MobileDownloadPage({
  url,
  onBack,
  onOpenUrl,
  copyText,
}: {
  url: string;
  onBack: () => void;
  onOpenUrl?: (url: string) => void;
  copyText?: (text: string) => Promise<void>;
}) {
  const [copied, setCopied] = useState(false);
  const mounted = useMountedRef();
  const showToast = useToast();
  const copy = () => {
    if (!copyText) return;
    copyText(url).then(
      () => {
        if (!mounted.current) return;
        setCopied(true);
        showToast("已复制下载链接");
      },
      () => {
        if (mounted.current) showToast("没有复制成功，请重试");
      },
    );
  };
  const open = onOpenUrl ? () => onOpenUrl(url) : undefined;

  return (
    <div className={account.subpage}>
      <MeSubpageHeader title="其他平台下载" onBack={onBack} />
      <div className={account.mePage}>
        <div className={account.downloadHero}>
          <span className={account.downloadHeroTile} aria-hidden="true">
            <FluentIcon name="link" size={24} />
          </span>
          <span className={account.downloadHeroText}>
            <span className={account.downloadHeroTitle}>在电脑上打开</span>
            <span className={account.downloadHeroUrl}>{displayUrl(url)}</span>
          </span>
          {copyText && (
            <button
              type="button"
              className={account.downloadPill}
              aria-label={copied ? "已复制下载页链接" : "复制下载页链接"}
              onClick={copy}
            >
              {copied ? "已复制" : "复制链接"}
            </button>
          )}
        </div>
        <PlatformGroup title="电脑" platforms={desktopPlatforms} onOpen={open} />
        <PlatformGroup title="手机和平板" platforms={mobilePlatforms} onOpen={open} />
      </div>
    </div>
  );
}
