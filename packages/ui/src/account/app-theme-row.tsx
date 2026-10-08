import { useEffect, useReducer, useState } from "react";
import { ActionSheet } from "../core/action-sheet";
import { FluentIcon } from "../core/fluent-icons";
import type { AppThemeCatalogEntry, AppThemeClient } from "../core/host-contracts";
import { NavRow } from "../core/platform-controls";
import { useToast } from "../core/toast";

/** 面板中列出的主题：季节主题会注明它会自己变化。 */
function optionLabel(entry: AppThemeCatalogEntry): string {
  return entry.seasonal ? `${entry.title}（自动）` : entry.title;
}

/**
 * 「应用主题」行末尾的值，写法与 Android 的 `AppThemeSheet.summary` 相同：水杉四季显示当前所画的季节（四季 · 秋杉），固定主题显示自己的标题。宿主还无法解析季节时，季节主题显示其普通标题。
 */
export function appThemeSummary(client: AppThemeClient): string {
  const catalog = client.catalog();
  const id = client.load();
  const entry = catalog.find((item) => item.id === id);
  if (!entry) return "";
  if (!entry.seasonal) return entry.title;
  // 季节与外观无关，所以浅色的解析结果对两者都适用。
  const season = client.resolve(false)?.season;
  const seasonTitle = catalog.find((item) => !item.seasonal && item.season === season)?.title;
  return seasonTitle ? `四季 · ${seasonTitle}` : entry.title;
}

/**
 * HarmonyOS 手机上的「我的 → 应用主题」：该行显示当前应用主题，点开是设计稿中的操作面板，列出「水杉四季（自动）」和四个固定季节。选中后通过宿主保存，宿主经由订阅给整个页面重新着色；该行也跟随同一订阅，所以页面打开期间季节更替也会在这里体现。
 */
export function AppThemeRow({ client, disabled }: { client: AppThemeClient; disabled?: boolean }) {
  const [open, setOpen] = useState(false);
  const [, refresh] = useReducer((count: number) => count + 1, 0);
  const showToast = useToast();
  useEffect(() => client.subscribe(refresh), [client]);

  const catalog = client.catalog();
  const current = client.load();
  const choose = (value: string) => {
    const entry = catalog.find((item) => item.id === value);
    if (!entry || entry.id === current) return;
    if (client.save(entry.id)) showToast(`已切换为「${optionLabel(entry)}」`);
    else showToast("没有保存，请重试");
    refresh();
  };

  return (
    <>
      <NavRow
        variant="me"
        icon={<FluentIcon name="paint_brush" size={18} />}
        title="应用主题"
        value={appThemeSummary(client) || undefined}
        disabled={disabled || catalog.length === 0}
        onClick={() => setOpen(true)}
      />
      <ActionSheet
        open={open}
        title="应用主题"
        subtitle="四季会随季节自动更换配色"
        options={catalog.map((entry) => ({
          value: entry.id,
          label: optionLabel(entry),
          selected: entry.id === current,
        }))}
        onSelect={choose}
        onClose={() => setOpen(false)}
      />
    </>
  );
}
