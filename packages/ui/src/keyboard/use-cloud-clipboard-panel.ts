import { useEffect, useState } from "react";
import { useLatestRef } from "../core/use-latest-ref";
import { useAsyncGeneration } from "../settings/use-async-generation";
import {
  CLOUD_CLIPBOARD_MAX_UTF16,
  cloudClipboardErrorMessage,
  cloudClipboardFailure,
} from "../settings/cloud-clipboard-send";
import { usePanelAction } from "./use-panel-action";
import {
  cloudClipboardDeviceStatus,
  cloudClipboardE2e,
  cloudClipboardItems,
  type CloudClipboardDeviceShape,
  type CloudClipboardDeviceStatusShape,
  type CloudClipboardE2eShape,
} from "./cloud-response";
import type {
  CloudClipboardAction,
  CloudClipboardItem,
  CloudClipboardResponse,
  CloudClipboardPanelClient,
} from "./panel-client";

/** 云剪贴板面板里等待用户确认的操作：关闭、重置或移除一台设备，三者都会清掉云端条目。 */
type CloudClipboardConfirmation =
  | { kind: "disable" }
  | { kind: "reset" }
  | { kind: "remove"; device: CloudClipboardDeviceShape };

function cloudClipboardConfirmationText(confirmation: CloudClipboardConfirmation): {
  label: string;
  message: string;
  action: string;
} {
  switch (confirmation.kind) {
    case "disable":
      return {
        label: "关闭云剪贴板",
        message:
          "关闭云剪贴板会删除云端历史和全部设备登记；重新开启后，其他设备都需要重新允许。是否继续？",
        action: "确认关闭并删除历史",
      };
    case "reset":
      return {
        label: "重置云剪贴板",
        message:
          "重置后，云端的全部条目会被清空，本设备换用一把新密钥；其他所有设备都需要在本设备上重新允许，才能继续使用云剪贴板。是否继续？",
        action: "确认重置",
      };
    case "remove":
      return {
        label: "移除设备",
        message: `移除「${confirmation.device.name || "未命名设备"}」后，云端的全部条目会被清空并换用新密钥；其他所有设备（包括仍在使用的）都需要在本设备上重新允许。是否继续？`,
        action: "确认移除",
      };
  }
}

/** 云剪贴板面板的列表、草稿、设备管理与确认会话；调用方决定界面和操作许可。 */
export function useCloudClipboardPanel(client: CloudClipboardPanelClient) {
  const [inputAvailable, setInputAvailable] = useState(
    Boolean(client.sendText && !client.canSendText),
  );
  const [confirmation, setConfirmation] = useState<CloudClipboardConfirmation | null>(null);
  const [loaded, setLoaded] = useState(false);
  const [search, setSearch] = useState("");
  const [items, setItems] = useState<CloudClipboardItem[]>([]);
  const [draft, setDraft] = useState("");
  const [enabled, setEnabled] = useState(true);
  const [e2e, setE2e] = useState<CloudClipboardE2eShape | null>(null);
  const [devices, setDevices] = useState<CloudClipboardDeviceStatusShape | null>(null);
  const [showDevices, setShowDevices] = useState(false);
  const [notice, setNotice] = useState("只上传你明确选择的内容");
  const {
    busy,
    busyRef,
    revisionRef: refreshRevision,
    run,
    invalidate,
  } = usePanelAction(setNotice);
  const draftRevision = useAsyncGeneration();
  const searchRef = useLatestRef(search);
  const showDevicesRef = useLatestRef(showDevices);
  const cloudGeneration = useAsyncGeneration(client);

  async function load(revision: number, nextSearch: string) {
    let result;
    try {
      result = await client.request({ operation: "list", search: nextSearch });
    } catch (error) {
      if (revision === refreshRevision.current) {
        setItems([]);
        setLoaded(false);
        setE2e(null);
        setDevices(null);
        setConfirmation(null);
      }
      throw error;
    }
    if (revision !== refreshRevision.current) return;
    const nextE2e = cloudClipboardE2e(result);
    setItems(cloudClipboardItems(result));
    setE2e(nextE2e);
    setLoaded(true);
    if (typeof result.enabled === "boolean") setEnabled(result.enabled);
    setNotice("云剪贴板已刷新");
    // 有设备在等本机允许，或用户打开了设备列表时，再取一次设备状态；取不到不影响条目。
    if (nextE2e?.status === "ready" && (nextE2e.pending_devices > 0 || showDevicesRef.current)) {
      try {
        const status = cloudClipboardDeviceStatus(await client.request({ operation: "devices" }));
        if (revision === refreshRevision.current) setDevices(status);
      } catch {
        if (revision === refreshRevision.current) setDevices(null);
      }
    } else {
      setDevices(null);
    }
  }

  function refresh(nextSearch = searchRef.current) {
    return run(async (revision) => {
      try {
        await load(revision, nextSearch);
      } catch (error) {
        // 错误码各有说明（例如自建中继拒绝了令牌、本设备待批准），认不出的仍说服务访问不了。
        if (revision === refreshRevision.current)
          setNotice(cloudClipboardErrorMessage(error, "无法访问云剪贴板服务"));
      }
    }, "无法访问云剪贴板服务");
  }

  useEffect(() => {
    const generation = cloudGeneration.current;
    busyRef.current = false;
    setItems([]);
    setLoaded(false);
    setE2e(null);
    setDevices(null);
    setConfirmation(null);
    setInputAvailable(Boolean(client.sendText && !client.canSendText));
    if (client.canSendText)
      void client
        .canSendText()
        .then((available) => {
          if (generation === cloudGeneration.current)
            setInputAvailable(available && Boolean(client.sendText));
        })
        .catch(() => {
          if (generation === cloudGeneration.current) setNotice("未能记录前台输入窗口");
        });
    if (client.rememberInputTarget)
      void client.rememberInputTarget().catch(() => {
        if (generation === cloudGeneration.current) setNotice("未能记录前台输入窗口");
      });
    void refresh(searchRef.current);
    return () => {
      invalidate();
    };
  }, [client, cloudGeneration, invalidate]);

  /** 发出一个可能失败的动作，失败时按错误码给出说明；成功后刷新条目，再显示 `success`。 */
  function act(
    action: CloudClipboardAction,
    success: string,
    failure: string,
    after?: (result: CloudClipboardResponse) => void,
  ) {
    return run(async (revision) => {
      let result: CloudClipboardResponse;
      try {
        result = await client.request(action);
      } catch (error) {
        if (revision !== refreshRevision.current) return;
        const message = cloudClipboardErrorMessage(error, failure);
        // 本设备其实还在等待允许，或者云剪贴板已被关闭：刷新一次，让面板换成对应的样子。
        if (cloudClipboardFailure(error) !== "unavailable") {
          try {
            await load(revision, searchRef.current);
          } catch {
            // 刷新失败时 load 已清空列表，这里只保留动作本身的失败说明。
          }
        }
        if (revision === refreshRevision.current) setNotice(message);
        return;
      }
      if (revision !== refreshRevision.current) return;
      after?.(result);
      try {
        await load(revision, searchRef.current);
      } catch {
        if (revision === refreshRevision.current)
          setNotice(`${success}，但历史刷新失败，请点击刷新`);
        return;
      }
      if (revision === refreshRevision.current) setNotice(success);
    }, failure);
  }

  function add() {
    if (!enabled || draft.trim().length === 0 || draft.length > CLOUD_CLIPBOARD_MAX_UTF16) {
      if (draft.trim().length === 0) setNotice("请输入要上传的文本");
      else if (draft.length > CLOUD_CLIPBOARD_MAX_UTF16)
        setNotice(`上传内容不能超过 ${CLOUD_CLIPBOARD_MAX_UTF16} 个 UTF-16 单元`);
      return;
    }
    const uploadedRevision = draftRevision.current;
    return act(
      { operation: "add", text: draft },
      "已上传",
      "上传失败，请确认账号 provider 已连接",
      () => {
        if (uploadedRevision === draftRevision.current) setDraft("");
      },
    );
  }

  function remove(id: string) {
    return act({ operation: "delete", id }, "已删除", "删除失败");
  }

  function deviceAction(action: CloudClipboardAction, success: string, failure: string) {
    return act(action, success, failure, (result) => {
      const status = cloudClipboardDeviceStatus(result);
      if (status) setDevices(status);
    });
  }

  function toggle(confirmed = false) {
    if (enabled && !confirmed) {
      setConfirmation({ kind: "disable" });
      return;
    }
    setConfirmation(null);
    const next = !enabled;
    return run(async (revision) => {
      const result = await client.request({ operation: "set_enabled", enabled: next });
      if (revision !== refreshRevision.current) return;
      const effective = typeof result.enabled === "boolean" ? result.enabled : next;
      setEnabled(effective);
      if (!effective) {
        setItems([]);
        setE2e(null);
        setDevices(null);
      } else {
        // 刚开启时本机要先登记，可能直接可用，也可能要等另一台设备允许。
        try {
          await load(revision, searchRef.current);
        } catch {
          // 刷新失败只影响列表，开关本身已经生效。
        }
      }
      if (revision === refreshRevision.current)
        setNotice(effective ? "云剪贴板已开启" : "云剪贴板已关闭");
    }, "更新云剪贴板设置失败");
  }

  function confirm() {
    const current = confirmation;
    if (!current) return;
    if (current.kind === "disable") return toggle(true);
    setConfirmation(null);
    if (current.kind === "reset")
      return deviceAction(
        { operation: "reset" },
        "云剪贴板已重置，其他设备需要重新允许",
        "重置云剪贴板失败",
      );
    return deviceAction(
      { operation: "remove", device_id: current.device.device_id },
      "已移除设备，其他设备需要重新允许",
      "移除设备失败",
    );
  }

  function toggleDevices() {
    const next = !showDevices;
    setShowDevices(next);
    if (!next) return;
    return run(async (revision) => {
      const status = cloudClipboardDeviceStatus(await client.request({ operation: "devices" }));
      if (revision !== refreshRevision.current) return;
      setDevices(status);
      setNotice("设备列表已刷新");
    }, "无法读取设备列表");
  }

  function choose(item: CloudClipboardItem, copyOnly = false) {
    copyOnly = copyOnly || !inputAvailable;
    const send = copyOnly ? client.copyText : client.sendText;
    if (!send) {
      setNotice("当前宿主未提供此操作");
      return;
    }
    return run(
      async (revision) => {
        await send(item.text);
        if (revision === refreshRevision.current)
          setNotice(copyOnly ? "已复制到本机剪贴板" : `已输入：${item.text}`);
      },
      copyOnly ? "复制失败，请重试" : "提交失败，可复制后手动粘贴",
    );
  }

  const pending = e2e?.status === "pending";
  const ready = e2e?.status === "ready";
  const waiting = devices?.devices.filter((device) => !device.approved && !device.current) ?? null;
  const waitingCount = waiting ? waiting.length : (e2e?.pending_devices ?? 0);
  const confirmText = confirmation && cloudClipboardConfirmationText(confirmation);

  function updateDraft(value: string) {
    draftRevision.current++;
    setDraft(value);
  }

  return {
    confirmation,
    setConfirmation,
    loaded,
    search,
    setSearch,
    items,
    draft,
    updateDraft,
    enabled,
    e2e,
    devices,
    showDevices,
    notice,
    busy,
    refresh,
    add,
    remove,
    deviceAction,
    toggle,
    confirm,
    toggleDevices,
    choose,
    pending,
    ready,
    waiting,
    waitingCount,
    confirmText,
  };
}

export type CloudClipboardPanelState = ReturnType<typeof useCloudClipboardPanel>;
