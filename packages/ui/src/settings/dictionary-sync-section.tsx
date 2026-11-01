import { useEffect, useRef } from "react";
import { GroupList, Row } from "../core/platform-controls";
import { ErrorAlert } from "../core/error-alert";
import { formatDateTime } from "../core/format-date";
import type { CloudClipboardDeviceShape } from "../keyboard/cloud-response";
import { ActionButton } from "./action-button";
import { EndpointSettingRow } from "./endpoint-setting-row";
import { SecretSettingRow } from "./secret-setting-row";
import { SettingsGroupNote } from "./settings-group-note";
import { SettingsManagerActions } from "./settings-manager-actions";
import { SettingsManagerBlock } from "./settings-manager-block";
import { useSettingsForm } from "./settings-form-context";
import * as settings from "./settings-style";
import {
  dictionarySyncBudgetWaitMessage,
  dictionarySyncDeviceGroups,
  dictionarySyncErrorMessage,
  withDictionarySyncWebDav,
} from "./dictionary-sync";
import { useDictionarySyncForm } from "./use-dictionary-sync-form";
import type { DictionarySyncModel } from "./use-dictionary-sync";

/** 本组开头的说明：同步什么、怎么加密、设备怎么批准。 */
export const DICTIONARY_SYNC_NOTE =
  "把「我的词条」存到自己的 WebDAV 网盘，在几台设备之间同步，删除也会同步；打字学到的顺序和权重不同步。词条在本机加密，网盘只保存密文；新设备要在已有设备上核对后点「允许」，与云剪贴板分开允许。";

/** 网盘字段下面的说明：什么时候同步、坚果云的配额、凭据和回收站。 */
export const DICTIONARY_SYNC_QUOTA_NOTE =
  "改词后约一分钟发送，设置打开期间每 30 分钟拉取一次，也可以随时点「立即同步」；坚果云免费账号每 30 分钟 600 次请求与其他应用共用，词库同步每台设备自动最多用 40 次。建议使用专用应用密码，移除设备后更换应用密码；已删除的密文可能留在网盘回收站。";

function deviceName(device: CloudClipboardDeviceShape): string {
  return device.name || "未命名设备";
}

/** 设备登记时间，读不出来时原样显示。 */
function registeredAt(value: string): string {
  const time = Date.parse(value);
  return Number.isNaN(time) ? value : formatDateTime(time);
}

/** 「词库」页的「词库同步」：网盘地址、账号和应用密码（测试连接通过才保存），同步状态与「立即同步」，被挡下的大批删除，以及词库密钥串的设备管理。宿主不提供 `dictionarySync` 时整组不显示。 */
export function DictionarySyncSection() {
  const { dictionarySync, draft, setDraft, busy } = useSettingsForm();
  if (!dictionarySync) return null;
  return (
    <>
      <DictionarySyncSettings
        model={dictionarySync}
        disabled={busy}
        saved={draft.dictionary_sync_webdav}
        onChange={(webdav) =>
          setDraft((current) => (current ? withDictionarySyncWebDav(current, webdav) : current))
        }
      />
      {dictionarySync.configured && <DictionarySyncDevices model={dictionarySync} />}
    </>
  );
}

function DictionarySyncSettings({
  model,
  disabled,
  saved,
  onChange,
}: {
  model: DictionarySyncModel;
  disabled: boolean;
  saved: Parameters<typeof withDictionarySyncWebDav>[1];
  onChange: (webdav: Parameters<typeof withDictionarySyncWebDav>[1]) => void;
}) {
  const { confirm } = useSettingsForm();
  const form = useDictionarySyncForm({ saved, probe: model.probe, disabled, onChange });
  const { status } = model;
  const error = model.error ?? status?.last_error ?? null;
  const budgetWait = dictionarySyncBudgetWaitMessage(status?.budget_wait_until ?? null);
  const actionsDisabled = disabled || model.busy;
  return (
    <GroupList title="词库同步">
      <SettingsGroupNote>{DICTIONARY_SYNC_NOTE}</SettingsGroupNote>
      <EndpointSettingRow
        title="服务器地址"
        inputLabel="词库同步服务器地址"
        description="坚果云填 https://dav.jianguoyun.com/dav/ 即可，水杉会在里面自动建 msime-dictionary 文件夹；其他网盘填它给的 WebDAV 地址。必须是 HTTPS，证书要由公网信任的机构签发。可以与云剪贴板用同一个网盘。"
        placeholder="https://dav.jianguoyun.com/dav/"
        value={form.url}
        disabled={disabled}
        onChange={form.editUrl}
      />
      {form.url !== "" && form.urlIssue && (
        <SettingsGroupNote role="status">{form.urlIssue}</SettingsGroupNote>
      )}
      <EndpointSettingRow
        title="账号"
        inputLabel="词库同步账号"
        description="网盘的登录账号，坚果云是登录邮箱。"
        value={form.username}
        disabled={disabled}
        onChange={form.editUsername}
      />
      {form.username !== "" && form.usernameIssue && (
        <SettingsGroupNote role="status">{form.usernameIssue}</SettingsGroupNote>
      )}
      <SecretSettingRow
        title="应用密码"
        label="词库同步应用密码"
        description="在网盘的安全设置里为第三方应用生成的密码，不是登录密码。只存在本机的设置里，不会随设置文件导出或账号同步。"
        value={form.password}
        placeholder={form.hasSavedPassword ? "留空以保留已保存的应用密码" : undefined}
        disabled={disabled}
        onChange={form.editPassword}
      />
      {form.password !== "" && form.passwordIssue && (
        <SettingsGroupNote role="status">{form.passwordIssue}</SettingsGroupNote>
      )}
      <SettingsGroupNote>{DICTIONARY_SYNC_QUOTA_NOTE}</SettingsGroupNote>
      <SettingsManagerBlock>
        <SettingsManagerActions>
          <ActionButton
            label="测试连接"
            ariaLabel="测试词库同步连接"
            disabled={!form.canProbe}
            action={form.probe}
          />
          <ActionButton
            label="保存"
            ariaLabel="保存词库同步设置"
            disabled={!form.canSave}
            action={form.save}
          />
          {form.configured && (
            <ActionButton
              label="停止同步"
              disabled={!form.canStop}
              action={async () => {
                const confirmed = await confirm({
                  title: "停止词库同步",
                  message:
                    "本机会清除保存的网盘地址、账号和应用密码，不再同步。本机的词条和网盘上的内容都不受影响，以后重新填写同一个网盘就能接着同步。",
                  confirmLabel: "停止同步",
                });
                if (confirmed) form.stop();
              }}
            />
          )}
        </SettingsManagerActions>
      </SettingsManagerBlock>
      {form.probeState.kind === "probing" && (
        <SettingsGroupNote role="status">正在测试连接……</SettingsGroupNote>
      )}
      {form.probeState.kind === "passed" && (
        <SettingsGroupNote role="status">连接正常，可以保存。</SettingsGroupNote>
      )}
      {form.probeState.kind === "failed" && (
        <SettingsGroupNote role="status">{form.probeState.message}</SettingsGroupNote>
      )}
      {model.configured && (
        <SettingsManagerBlock aria-label="词库同步状态">
          <p className={settings.managerNote} role="status">
            {model.syncing
              ? "正在同步……"
              : status?.last_success_ms
                ? `上次同步：${formatDateTime(status.last_success_ms)}`
                : "还没有同步过。"}
          </p>
          {status && status.joined && (
            <p className={settings.managerNote}>
              共享词库有 {status.words} 个词条；待发送 {status.pending_upload} 项，待写入本机{" "}
              {status.pending_apply} 项。
            </p>
          )}
          {budgetWait && <p className={settings.managerNote}>{budgetWait}</p>}
          {error && <ErrorAlert>{dictionarySyncErrorMessage(error)}</ErrorAlert>}
          {error === "dictionary_sync_rollback" && (
            <ActionButton
              label="确认网盘现状并恢复同步"
              disabled={actionsDisabled || model.syncing}
              action={() =>
                model.trustRemote(() =>
                  confirm({
                    title: "确认网盘现状并恢复同步",
                    message:
                      "仅在你自己清理过网盘文件夹时使用。会保留当前本机词条，但忘掉同步位置和待发改动；未上传的删除可能从网盘重新合并回来。",
                    confirmLabel: "恢复同步",
                    danger: true,
                  }),
                )
              }
            />
          )}
          {status?.clock_skew && (
            <p className={settings.managerNote}>
              收到的改动里有时间比本机快一天以上，请检查各台设备的系统时间。
            </p>
          )}
          {status?.device_id && status.code && !status.approved && (
            <div className={settings.syncPrompt} aria-label="本设备等待允许">
              <p className={settings.managerNote}>
                本设备还没有获准同步词库。在一台已在同步词库的设备上打开「词库」设置里的「词库同步」，确认那里显示的核对码与下面完全一致后点「允许」，再回到这里点「立即同步」。
              </p>
              <p className={settings.syncCode} aria-label="本设备核对码">
                {status.code}
              </p>
            </div>
          )}
          {status?.held_deletions != null && (
            <div className={settings.syncPrompt} aria-label="被挡下的删除">
              <p className={settings.managerNote}>
                本机比上次同步时少了 {status.held_deletions}{" "}
                个词条，这些删除还没有发到网盘。如果是你自己删的，选「同步删除」，其他设备上也会删掉；如果是本机词库被重置或损坏了，选「从网盘恢复」把它们写回本机。
              </p>
              <SettingsManagerActions>
                <ActionButton
                  label="同步删除"
                  className="secondary danger-button"
                  disabled={actionsDisabled}
                  action={async () => {
                    const confirmed = await confirm({
                      title: "同步删除",
                      message: `其他设备上的这 ${status.held_deletions} 个词条也会被删除。`,
                      confirmLabel: "同步删除",
                      danger: true,
                    });
                    if (confirmed) await model.resolveDeletions("confirm");
                  }}
                />
                <ActionButton
                  label="从网盘恢复"
                  disabled={actionsDisabled}
                  action={() => model.resolveDeletions("restore")}
                />
              </SettingsManagerActions>
            </div>
          )}
          <SettingsManagerActions>
            <ActionButton
              label="立即同步"
              disabled={actionsDisabled || model.syncing}
              ariaBusy={model.syncing}
              action={model.syncNow}
            />
          </SettingsManagerActions>
          {model.notice && (
            <p className={settings.managerNote} role="status">
              {model.notice}
            </p>
          )}
        </SettingsManagerBlock>
      )}
    </GroupList>
  );
}

/** 词库密钥串的设备：等待允许的（允许之前对照核对码）、已允许的（本机以外可以移除），以及很少用到的重置密钥和以本机为准重新开始。每台设备只列一条。 */
function DictionarySyncDevices({ model }: { model: DictionarySyncModel }) {
  const { confirm, busy } = useSettingsForm();
  const disabled = busy || model.busy;
  const { devices, status } = model;
  const registered = Boolean(status?.device_id);
  // 本机还没获准时手里没有密钥，允许、拒绝和移除别的设备都做不了，只列出设备；重置和重新开始仍在。
  const canManage = status?.approved !== false;
  // 已经在网盘上登记过的设备打开本页时取一次设备列表，等待允许的设备才看得到；还没登记的等第一次同步。
  const loaded = useRef(false);
  useEffect(() => {
    if (loaded.current || !registered || devices) return;
    loaded.current = true;
    void model.loadDevices(true);
  }, [registered, devices, model]);
  const groups = devices ? dictionarySyncDeviceGroups(devices.devices) : null;
  return (
    <GroupList title="词库同步设备">
      {!groups ? (
        <SettingsManagerBlock>
          <p className={settings.managerNote}>
            {model.devicesLoading
              ? "正在读取设备……"
              : registered
                ? "点「查看设备」列出同步词库的设备和等待允许的设备。"
                : "第一次同步之后，这里会列出同步词库的设备和等待允许的设备。"}
          </p>
          <SettingsManagerActions>
            <ActionButton
              label="查看设备"
              disabled={disabled || model.devicesLoading}
              action={() => model.loadDevices()}
            />
          </SettingsManagerActions>
        </SettingsManagerBlock>
      ) : (
        <>
          {canManage && groups.waiting.length > 0 && (
            <>
              <SettingsGroupNote role="status">
                有 {groups.waiting.length}{" "}
                台设备请求同步词库。核对两台设备上的核对码一致再允许；不认识的设备请拒绝。允许后，这台设备就能读写网盘上的词库。
              </SettingsGroupNote>
              {groups.waiting.map((device) => (
                <Row
                  key={device.device_id}
                  title={deviceName(device)}
                  description={
                    <>
                      {registeredAt(device.created_at)} 登记 · 核对码{" "}
                      <span
                        className={settings.syncCodeInline}
                        aria-label={`${deviceName(device)} 的核对码`}
                      >
                        {device.code}
                      </span>
                    </>
                  }
                >
                  <SettingsManagerActions as="span">
                    <ActionButton
                      label="允许"
                      ariaLabel={`允许 ${deviceName(device)}`}
                      disabled={disabled}
                      action={() => model.approve(device.device_id)}
                    />
                    <ActionButton
                      label="拒绝"
                      ariaLabel={`拒绝 ${deviceName(device)}`}
                      disabled={disabled}
                      action={() => model.reject(device.device_id)}
                    />
                  </SettingsManagerActions>
                </Row>
              ))}
            </>
          )}
          {groups.approved.map((device) => (
            <Row
              key={device.device_id}
              title={`${deviceName(device)}${device.current ? "（本机）" : ""}`}
              description={`${registeredAt(device.created_at)} 登记`}
            >
              {canManage && !device.current && (
                <ActionButton
                  label="移除"
                  ariaLabel={`移除 ${deviceName(device)}`}
                  className="secondary danger-button"
                  disabled={disabled}
                  action={async () => {
                    const confirmed = await confirm({
                      title: "移除设备",
                      message: `移除「${deviceName(device)}」后会更换密钥，其他设备都需要在本机上重新允许。被移除的设备不再收到之后的改动；建议随后在网盘上更换应用密码。`,
                      confirmLabel: "移除",
                      danger: true,
                    });
                    if (confirmed) await model.remove(device.device_id);
                  }}
                />
              )}
            </Row>
          ))}
          <SettingsManagerBlock>
            <SettingsManagerActions>
              <ActionButton
                label="刷新设备"
                disabled={disabled || model.devicesLoading}
                action={() => model.loadDevices()}
              />
            </SettingsManagerActions>
          </SettingsManagerBlock>
        </>
      )}
      <SettingsManagerBlock>
        <p className={settings.managerNote}>
          很少用到：找不到可以用来允许本机的设备时，可以重置密钥，其他设备都需要重新允许；持有旧密钥的设备都不在了时，可以从这台设备重新开始，以本机的词条为准建一份新的共享词库，其他设备获准后会把它们的词合并进来，不会因此删词。
        </p>
        <SettingsManagerActions>
          <ActionButton
            label="重置密钥"
            className="secondary danger-button"
            disabled={disabled}
            action={async () => {
              const confirmed = await confirm({
                title: "重置词库同步密钥",
                message: "会换一把只给本机的新密钥，其他设备都需要在本机上重新允许。",
                confirmLabel: "重置密钥",
                danger: true,
              });
              if (confirmed) await model.reset();
            }}
          />
          <ActionButton
            label="从这台设备重新开始"
            className="secondary danger-button"
            disabled={disabled}
            action={async () => {
              const confirmed = await confirm({
                title: "从这台设备重新开始",
                message:
                  "以本机的词条为准，在网盘上重新建一份共享词库。网盘上原有的记录不再读取；其他设备获准后会把它们的词合并进来，不会因此删词。",
                confirmLabel: "重新开始",
                danger: true,
              });
              if (confirmed) await model.restart();
            }}
          />
        </SettingsManagerActions>
      </SettingsManagerBlock>
    </GroupList>
  );
}
