import { errorCode } from "../core/error-code";
import { SettingsGroupNote } from "./settings-group-note";
import { HostActionButton } from "../keyboard/HostActionButton";
import { GroupList, Row } from "../core/platform-controls";

export interface InputMethodServiceSectionProps {
  visible: boolean;
  macos: boolean;
  linux: boolean;
  restartInputMethod?: () => Promise<void>;
  installInputSource?: () => Promise<void>;
  onRepairFinished?: () => Promise<void>;
  /** macOS：生效的输入法装在当前用户的目录里，安全输入期间会被系统停用。 */
  userOnlyInstall?: boolean;
  /** macOS：把用户目录里的那份换成系统目录里的一份，宿主为此请求管理员授权。 */
  migrateToSystem?: () => Promise<void>;
}

/** 桌面输入法服务的修复与重启操作。 */
export function InputMethodServiceSection({
  visible,
  macos,
  linux,
  restartInputMethod,
  installInputSource,
  onRepairFinished,
  userOnlyInstall,
  migrateToSystem,
}: InputMethodServiceSectionProps) {
  if (!visible) return null;

  // macOS 只给一个「修复输入源」：原来并排的「重新注册当前输入源」和「安装或更新水杉输入源」要用户猜该按哪个，应用启动时也已经自动安装或更新过输入源。修复先按版本补装输入源（没装时安装、随附的更新时替换，从不降级；不是打包运行、没有随附的输入源时宿主跳过这一步），再重新注册并启用它。安装失败也照样重新注册已装的那份，原来单独的「重新注册」就能修好的情况不能因为安装失败而修不了；注册完再把安装失败报出来。
  const repairInputSource =
    macos && installInputSource && restartInputMethod
      ? async () => {
          try {
            let installFailed = false;
            let installFailure: unknown;
            try {
              await installInputSource();
            } catch (error) {
              if (errorCode(error) === "cancelled") throw error;
              installFailed = true;
              installFailure = error;
            }
            await restartInputMethod();
            if (installFailed) throw installFailure;
          } finally {
            await onRepairFinished?.();
          }
        }
      : restartInputMethod;

  return (
    <GroupList title="输入法服务">
      <SettingsGroupNote>
        {macos
          ? "输入源没装或比应用随附的旧时先装上随附的版本（已装的更新时保留不动），再重新注册并启用它；输入法从输入法菜单里消失或切换不过去时使用。系统目录安装会请求管理员权限；取消后可重试。当前输入法进程继续按系统生命周期运行。"
          : linux
            ? "重启 IBus 输入法服务；使用 Fcitx5 时重载水杉插件，关闭并重建所有输入会话，不影响其他输入法。"
            : "请求受监督的输入法服务重新启动。"}
      </SettingsGroupNote>
      {macos && userOnlyInstall && migrateToSystem && (
        <SettingsGroupNote>
          迁移到系统目录：装在用户目录的输入法在输入密码等安全输入期间会被 macOS
          停用，在输入法菜单里变灰。迁移后不再受影响，输入法列表、词库和偏好都保留。
        </SettingsGroupNote>
      )}
      {macos && userOnlyInstall && migrateToSystem && (
        <Row title="迁移到系统目录">
          <HostActionButton
            action={async () => {
              try {
                await migrateToSystem();
              } finally {
                await onRepairFinished?.();
              }
            }}
            label="迁移"
            success="已迁移到系统目录。"
            cancelled="已取消管理员授权，输入法仍在原来的位置。可以重新点击迁移。"
            error="迁移没有完成，输入法仍在原来的位置，请稍后重试。"
          />
        </Row>
      )}
      <Row title={macos ? "修复输入源" : "立即重启输入法服务"}>
        <HostActionButton
          action={repairInputSource}
          label={macos ? "修复" : "重启"}
          success={macos ? "输入源已修复。" : linux ? "已请求重启输入法服务。" : "已发送重启请求。"}
          cancelled="已取消管理员授权，输入源未完成修复。可以重新点击修复。"
          error={
            macos
              ? "修复输入源失败，请确认输入法已经安装后重试。"
              : "重启输入法服务失败，请稍后重试。"
          }
        />
      </Row>
    </GroupList>
  );
}
