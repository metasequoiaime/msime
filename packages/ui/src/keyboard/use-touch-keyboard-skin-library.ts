import { useEffect, useState } from "react";
import { useAsyncActionRunner } from "../core/use-async-action";
import { libraryError } from "./touch-keyboard-skin-errors";
import {
  boundedSkinName,
  type CustomSkinLibraryAction,
  type CustomSkinLibraryClient,
  type SavedTouchKeyboardSkin,
  type TouchKeyboardSkinDesign,
} from "./touch-keyboard-skin-design";

export type TouchSkinLibraryNameEditor =
  | { operation: "create" }
  | { operation: "rename"; id: string };
export type TouchSkinLibraryConfirmation = {
  operation: "update" | "delete";
  item: SavedTouchKeyboardSkin;
};
export type TouchSkinLibrarySaveResult = "created" | "renamed";

/** 命名皮肤库的加载、表单与确认流程；布局、设计历史和使用设计由调用方持有。 */
export function useTouchKeyboardSkinLibrary(library?: CustomSkinLibraryClient) {
  const [saved, setSaved] = useState<SavedTouchKeyboardSkin[]>([]);
  const [notice, setNotice] = useState("");
  const [nameEditor, setNameEditor] = useState<TouchSkinLibraryNameEditor | null>(null);
  const [skinName, setName] = useState("");
  const [confirmation, setConfirmation] = useState<TouchSkinLibraryConfirmation | null>(null);
  const {
    busy,
    running,
    run: runLibraryAction,
  } = useAsyncActionRunner(setNotice, undefined, library);
  useEffect(() => {
    if (!library) return;
    void runLibraryAction(
      async (isCurrent) => {
        const items = await library.load();
        if (isCurrent()) setSaved(items);
      },
      { formatError: libraryError },
    );
  }, [library, runLibraryAction]);

  const mutate = async (action: CustomSkinLibraryAction, success: string) => {
    if (!library || running.current) return false;
    let succeeded = false;
    await runLibraryAction(
      async (isCurrent) => {
        const items = await library.mutate(action);
        if (!isCurrent()) return;
        setSaved(items);
        setNotice(success);
        succeeded = true;
      },
      { formatError: libraryError },
    );
    return succeeded;
  };
  const beginCreate = () => {
    setName(`我的设计 ${saved.length + 1}`);
    setNameEditor({ operation: "create" });
    setNotice("");
  };
  const beginRename = (item: SavedTouchKeyboardSkin) => {
    setName(item.name);
    setNameEditor({ operation: "rename", id: item.id });
    setNotice("");
  };
  const submitName = async (
    design: TouchKeyboardSkinDesign,
  ): Promise<TouchSkinLibrarySaveResult | undefined> => {
    if (!nameEditor) return;
    const name = skinName.trim();
    if (!name) {
      setNotice("请输入皮肤名称。");
      return;
    }
    const created = nameEditor.operation === "create";
    const action: CustomSkinLibraryAction = created
      ? { operation: "create", name, design }
      : { operation: "rename", id: nameEditor.id, name };
    if (
      await mutate(
        action,
        created ? "设计已保存到我的皮肤；保存页面设置后会应用到键盘。" : "皮肤名称已更新。",
      )
    ) {
      setNameEditor(null);
      return created ? "created" : "renamed";
    }
  };
  const confirmMutation = async (design: TouchKeyboardSkinDesign) => {
    if (!confirmation) return false;
    const { operation, item } = confirmation;
    const action: CustomSkinLibraryAction =
      operation === "update"
        ? { operation: "update", id: item.id, design }
        : { operation: "delete", id: item.id };
    const succeeded = await mutate(
      action,
      operation === "update" ? "已用当前设计更新这套皮肤。" : "已删除这套皮肤。",
    );
    if (succeeded) setConfirmation(null);
    return succeeded;
  };
  return {
    saved,
    busy,
    notice,
    setNotice,
    nameEditor,
    skinName,
    setSkinName: (value: string) => setName(boundedSkinName(value)),
    beginCreate,
    beginRename,
    cancelName: () => setNameEditor(null),
    submitName,
    confirmation,
    requestMutation: (
      operation: TouchSkinLibraryConfirmation["operation"],
      item: SavedTouchKeyboardSkin,
    ) => setConfirmation({ operation, item }),
    cancelConfirmation: () => setConfirmation(null),
    confirmMutation,
  };
}

export type TouchKeyboardSkinLibraryState = ReturnType<typeof useTouchKeyboardSkinLibrary>;
