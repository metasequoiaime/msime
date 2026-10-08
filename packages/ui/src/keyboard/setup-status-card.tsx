import { useLayoutEffect, useState } from "react";
import type { Preferences } from "../index";
import type { ImeSetupClient, ImeSetupState } from "../core/host-contracts";
import { MsimeMark } from "../core/brand-logo";
import { FluentIcon } from "../core/fluent-icons";
import { errorMessage } from "../core/error-message";
import { useToast } from "../core/toast";
import { touchKeyboardSchemeTitle } from "../settings/touch-keyboard-scheme-helpers";

/** 设置引导界面调用的宿主操作：两个设置步骤，以及宿主自己的设置状态。 */
export interface SetupActions {
  openSystemKeyboardSettings?: () => Promise<void>;
  showInputMethodPicker?: () => Promise<void>;
  setup?: ImeSetupClient;
}

/**
 * 宿主的设置状态：界面挂载时读一次，之后通过宿主的变更通知保持最新（比如从系统设置返回时）。宿主能回答之前为 null，没有设置客户端的宿主始终为 null。
 *
 * 先订阅再读取，两者之间发生的变化不会丢；两步都放在 layout effect 里，已经知道答案的宿主一帧也不会显示「检查中」。
 */
export function useImeSetupState(setup: ImeSetupClient | undefined): ImeSetupState | null {
  const [state, setState] = useState<{ client: ImeSetupClient; value: ImeSetupState | null }>();
  useLayoutEffect(() => {
    if (!setup) return;
    const unsubscribe = setup.subscribe((value) => setState({ client: setup, value }));
    setState({ client: setup, value: setup.read() });
    return unsubscribe;
  }, [setup]);
  // 从界面已不再持有的客户端读到的状态，不是这个宿主的答案。
  return state && state.client === setup ? state.value : null;
}

/** 执行一个宿主操作，失败时用 toast 报告，这是设计里表示点按没生效的方式。 */
function useHostAction() {
  const toast = useToast();
  return (action: (() => Promise<void>) | undefined) => {
    if (!action) return;
    void action().catch((error: unknown) => toast(errorMessage(error)));
  };
}

/** 把本输入法设为默认的操作。系统选择器只列出已启用的输入法，所以第一步完成前改为打开系统的输入法列表，与 Android 的 `ImeSetup.makeDefault` 一致。 */
export function makeDefaultAction(
  actions: SetupActions | undefined,
  state: ImeSetupState | null,
): (() => Promise<void>) | undefined {
  if (state?.enabled === false) return actions?.openSystemKeyboardSettings;
  return actions?.showInputMethodPicker;
}

const pendingBadge =
  "flex size-5 shrink-0 items-center justify-center rounded-full bg-[#FF9500] text-[12px] font-bold text-white";

/** 检查项前的 20px 标记：完成时是强调色对勾，待处理时是橙色的 !，宿主还没回答时是空圆环。 */
function CheckBadge({ done }: { done: boolean | null }) {
  if (done === true)
    return (
      <span
        className="flex size-5 shrink-0 items-center justify-center rounded-full bg-[var(--accent-color)] [color:var(--p-on-accent,#FFFFFF)]"
        aria-hidden="true"
      >
        <svg
          width="12"
          height="12"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="3"
          strokeLinecap="round"
          strokeLinejoin="round"
        >
          <path d="m5 12.5 4.5 4.5L19 7.5" />
        </svg>
      </span>
    );
  if (done === false)
    return (
      <span className={pendingBadge} aria-hidden="true">
        !
      </span>
    );
  return (
    <span
      className="size-5 shrink-0 rounded-full [box-shadow:inset_0_0_0_1.5px_var(--p-hair,rgba(127,127,127,0.3))]"
      aria-hidden="true"
    />
  );
}

function SetupCheck({
  label,
  done,
  actionLabel,
  onAction,
}: {
  label: string;
  done: boolean | null;
  actionLabel: string;
  onAction?: () => void;
}) {
  return (
    <div className="flex min-h-5 items-center gap-2.5 text-[14px]">
      <CheckBadge done={done} />
      <span className="min-w-0 flex-1">
        {label}
        <span className="sr-only">
          {done === true ? "，已完成" : done === false ? "，未完成" : ""}
        </span>
      </span>
      {onAction && (
        <button
          type="button"
          className="m-0 shrink-0 cursor-pointer border-0 bg-transparent p-0 text-[14px] [font-family:inherit] [color:var(--p-accent-text,var(--accent-color))] active:opacity-70"
          onClick={onAction}
        >
          {actionLabel}
        </button>
      )}
    </div>
  );
}

/**
 * 「设置」根页的状态卡片：水杉标志、当前方案下设置是否已完成、两项设置检查及每项还需的步骤，以及「试用键盘」。
 *
 * 宿主还没报告状态的检查项画空圆环、不提供步骤，标题下一行显示「检查中」。完全没有设置客户端的宿主永远不会报告，所以那里的检查项两个步骤都提供，那一行只显示方案，而不声称正在检查。
 */
export function SetupStatusCard({
  actions,
  preferences,
  onTry,
}: {
  actions?: SetupActions;
  preferences: Preferences;
  onTry: () => void;
}) {
  const state = useImeSetupState(actions?.setup);
  const run = useHostAction();
  const reporting = Boolean(actions?.setup);
  const enabled = state?.enabled ?? null;
  const current = state?.current ?? null;
  const scheme = touchKeyboardSchemeTitle(preferences);
  const status =
    enabled === true && current === true
      ? `已启用 · ${scheme}`
      : enabled === false || current === false
        ? `尚未完成设置 · ${scheme}`
        : reporting
          ? "检查中"
          : scheme;
  // 检查待处理时提供对应步骤；宿主无法回答时始终提供。
  const offered = (done: boolean | null) => done === false || (!reporting && done === null);
  const enable = actions?.openSystemKeyboardSettings;
  const makeDefault = makeDefaultAction(actions, state);

  return (
    <div
      className="flex flex-col gap-3.5 rounded-[20px] bg-[var(--p-group-bg)] p-4 [color:var(--p-text)]"
      role="group"
      aria-label="水杉输入法状态"
    >
      <div className="flex items-center gap-3">
        <span
          className="flex size-[52px] shrink-0 items-center justify-center rounded-full bg-[color-mix(in_srgb,var(--accent-color)_22%,var(--p-group-bg))] light-theme:bg-[color-mix(in_srgb,var(--accent-color)_14%,var(--p-group-bg))]"
          aria-hidden="true"
        >
          <MsimeMark size={30} />
        </span>
        <div className="flex min-w-0 flex-1 flex-col gap-0.5">
          <span className="text-[17px] font-semibold">水杉输入法</span>
          <span className="text-[13px] [color:var(--p-sub)]" role="status">
            {status}
          </span>
        </div>
      </div>
      <div className="flex flex-col gap-2">
        <SetupCheck
          label="已在系统中启用"
          done={enabled}
          actionLabel="去开启"
          onAction={offered(enabled) && enable ? () => run(enable) : undefined}
        />
        <SetupCheck
          label="设为默认输入法"
          done={current}
          actionLabel="设为默认"
          onAction={offered(current) && makeDefault ? () => run(makeDefault) : undefined}
        />
      </div>
      <button
        type="button"
        className="mt-1 flex h-10 w-full cursor-pointer items-center justify-center gap-1.5 rounded-[24px] border-0 bg-[var(--accent-soft)] p-0 text-[15px] font-semibold [font-family:inherit] [color:var(--accent-color)] active:opacity-70"
        onClick={onTry}
      >
        <FluentIcon name="keyboard" size={16} />
        试用键盘
      </button>
    </div>
  );
}

/**
 * 鸿蒙 2in1 的设置警告，由外壳画在「输入」和「关于」顶部：一个橙色的 !，说明还缺什么以及修复它的那一步。两项检查都通过时不渲染，任一项未知时也不渲染，所以无法判断的宿主不会误报。
 */
export function SetupWarningStrip({ actions }: { actions?: SetupActions }) {
  const state = useImeSetupState(actions?.setup);
  const run = useHostAction();
  if (!state || state.enabled === null || state.current === null) return null;
  if (state.enabled && state.current) return null;
  const step = state.enabled
    ? {
        text: "水杉输入法还不是默认输入法",
        label: "设为默认",
        action: actions?.showInputMethodPicker,
      }
    : {
        text: "水杉输入法尚未在「系统 → 输入法」中添加",
        label: "去添加",
        action: actions?.openSystemKeyboardSettings,
      };
  const { action } = step;
  return (
    <div
      className="mb-2 flex flex-wrap items-center gap-x-3 gap-y-2 rounded-[20px] border border-solid border-[rgba(255,149,0,0.3)] bg-[rgba(255,149,0,0.12)] px-3.5 py-2.5 text-[13px] light-theme:border-[#FFD8A8] light-theme:bg-[#FFF4E5]"
      role="status"
    >
      <span
        className="flex size-[18px] shrink-0 items-center justify-center rounded-full bg-[#FF9500] text-[12px] font-bold text-white"
        aria-hidden="true"
      >
        !
      </span>
      <span className="min-w-[200px] flex-1">{step.text}</span>
      {action && (
        <button
          type="button"
          className="m-0 cursor-pointer whitespace-nowrap rounded-[20px] border-0 bg-[var(--accent-color)] px-3 py-1 text-[12px] [font-family:inherit] [color:var(--p-on-accent,#FFFFFF)] active:opacity-70"
          onClick={() => run(action)}
        >
          {step.label}
        </button>
      )}
    </div>
  );
}
