import { SchemeTraits } from "../SchemeTraits";

import { TextPolicy } from "../TextPolicy";
/**
 * Candidate-management menu order and host operation metadata, ported from
 * platforms/android/java/app/msime/android/CandidateManagementAction.java.
 *
 * The menu item ids are derived from the declaration order, so the order is part of the contract with
 * whatever renders the menu.
 */
const MENU_ITEM_BASE: number = 1000;

/** What fits on one menu row on a phone; a gloss is a dictionary line and can be much longer. */
const MAX_GLOSS_TITLE_CHARACTERS: number = 24;

export interface ManagementAction {
  readonly id: string;
  readonly title: string;
  readonly announcement: string;
  readonly confirmationRequired: boolean;
  readonly menuItemId: number;
  readonly position?: number;
  readonly checked?: boolean;
  readonly available?: boolean;
  /** 「加入生词本」打开菜单时那一格的候选原文和释义。菜单开着时候选栏可能重建，点下去时拿它们和那一格现在的候选比，不一样就不收。 */
  readonly candidate?: string;
  readonly gloss?: string;
  /** 「朗读」打开菜单时共享层答的那段英文（`msime_client_read_aloud_text`）。 */
  readonly spoken?: string;
}

function action(
  index: number,
  id: string,
  title: string,
  announcement: string,
  confirmationRequired: boolean,
): ManagementAction {
  return {
    id: id,
    title: title,
    announcement: announcement,
    confirmationRequired: confirmationRequired,
    menuItemId: MENU_ITEM_BASE + index,
  };
}

const ACTIONS: ManagementAction[] = [
  action(0, "PROMOTE", "优先显示", "已优先显示", false),
  action(1, "FIX_FIRST", "固定到首位", "已固定到首位", false),
  action(2, "CLEAR_POSITION", "取消固定", "已取消固定", false),
  action(3, "REMOVE", "删除词条…", "已删除词条", true),
];

export class CandidateManagementAction {
  static readonly ACTIONS: ManagementAction[] = ACTIONS;
  static readonly PROMOTE: ManagementAction = ACTIONS[0];
  static readonly FIX_FIRST: ManagementAction = ACTIONS[1];
  static readonly CLEAR_POSITION: ManagementAction = ACTIONS[2];
  static readonly REMOVE: ManagementAction = ACTIONS[3];

  /** Windows exposes all five fixed slots and marks the currently held slot. */
  static actionsForFixedPosition(
    fixedPosition: number,
    available: boolean = true,
    includeRemove: boolean = true,
  ): ManagementAction[] {
    const actions: ManagementAction[] = [
      { ...CandidateManagementAction.PROMOTE, available: available },
    ];
    for (let position: number = 1; position <= 5; position++) {
      actions.push({
        id: `FIX_${position}`,
        title: `第 ${position} 位`,
        announcement: `已固定到第 ${position} 位`,
        confirmationRequired: false,
        menuItemId: MENU_ITEM_BASE + position,
        position: position,
        checked: fixedPosition === position,
        available: available,
      });
    }
    actions.push({
      ...CandidateManagementAction.CLEAR_POSITION,
      menuItemId: MENU_ITEM_BASE + 6,
      available: available && fixedPosition > 0,
    });
    if (includeRemove) {
      actions.push({
        ...CandidateManagementAction.REMOVE,
        menuItemId: MENU_ITEM_BASE + 7,
        available: available,
      });
    }
    return actions;
  }

  /** Windows omits deletion for a single Unicode code point. */
  static hasMultipleCodePoints(text: string): boolean {
    return TextPolicy.characters(text).length !== 1;
  }

  /** Dictionary mutations are only valid for local/user-dictionary candidates, in a scheme that learns into the main dictionary; the others' candidates are not in it to pin, demote or remove. */
  static candidateActionsAvailable(scheme: string, source: number): boolean {
    if (!SchemeTraits.learnsIntoMainDictionary(SchemeTraits.fromName(scheme))) {
      return false;
    }
    return source === 0 || source === 1 || source === 4;
  }

  static fromMenuItemId(itemId: number): ManagementAction {
    const index: number = itemId - MENU_ITEM_BASE;
    if (index < 0 || index >= ACTIONS.length) {
      throw new Error("Unknown candidate management action");
    }
    return ACTIONS[index];
  }

  static fixedPosition(entry: ManagementAction): number {
    if (entry !== CandidateManagementAction.FIX_FIRST) {
      throw new Error("Action does not fix a position");
    }
    return 1;
  }

  /**
   * The candidate's own gloss, offered as something to type instead of the candidate.
   *
   * macOS hands the translation over with Option or Control and a digit; a touch keyboard has no
   * modifiers, so the source puts it on the candidate's long press. There it is the only thing on
   * the menu and the title is the gloss alone — "输入" in front of it would state what the menu
   * already says. This host's menu carries the entry-management items too, inherited from the
   * Windows and Android side, so a bare gloss would read as one more noun among verbs and the
   * title says what tapping it does.
   *
   * This action-worded form sits beside the desktop management items. `touchGlossAction` below
   * removes that wording because the source's touch menu contains the gloss and nothing else.
   */
  static glossAction(gloss: string): ManagementAction | null {
    const text: string = gloss.trim();
    if (text.length === 0) {
      return null;
    }
    // A gloss is a dictionary line and can run long; the menu is one row on a phone.
    const shown: string[] = TextPolicy.characters(text);
    const title: string =
      shown.length > MAX_GLOSS_TITLE_CHARACTERS
        ? shown.slice(0, MAX_GLOSS_TITLE_CHARACTERS - 1).join("") + "…"
        : text;
    return {
      id: "INSERT_GLOSS",
      title: `输入“${title}”`,
      announcement: `已输入 ${text}`,
      confirmationRequired: false,
      menuItemId: MENU_ITEM_BASE + 8,
    };
  }

  /** Apple touch candidates expose only the gloss, with no desktop dictionary operations beside it. */
  static touchGlossAction(gloss: string): ManagementAction | null {
    const action: ManagementAction | null = CandidateManagementAction.glossAction(gloss);
    if (action === null) {
      return null;
    }
    const text: string = gloss.trim();
    const characters: string[] = TextPolicy.characters(text);
    const title: string =
      characters.length > MAX_GLOSS_TITLE_CHARACTERS
        ? characters.slice(0, MAX_GLOSS_TITLE_CHARACTERS - 1).join("") + "…"
        : text;
    return { ...action, title: title };
  }

  /**
   * 「加入生词本」：把候选和它显示着的释义收进背单词的生词本，排在菜单最后，和 Android、iOS 的长按菜单、桌面的候选右键菜单同一个位置。只在候选带着真正的译文时有；Engine 的注释（例如五笔编码）不是释义。提示由共享层给出，所以 `announcement` 是空的；打开菜单时的候选原文和释义记在 `candidate`、`gloss` 里。
   */
  static personalWordbookAction(
    gloss: string,
    isTranslation: boolean,
    candidate: string = "",
  ): ManagementAction | null {
    if (!isTranslation || gloss.trim().length === 0) {
      return null;
    }
    return {
      id: "ADD_TO_PERSONAL_WORDBOOK",
      title: "加入生词本",
      announcement: "",
      confirmationRequired: false,
      menuItemId: MENU_ITEM_BASE + 9,
      candidate: candidate,
      gloss: gloss.trim(),
    };
  }

  /**
   * 「朗读」：用系统语音读出共享层答的那段英文（显示着的英文释义的第一个义项，或候选本身是英文），排在「加入生词本」前面、和它同一组，和 Android、iOS 的长按菜单、桌面的候选右键菜单同一个位置。`spoken` 为 null（没有能读的，或设备没有文本转语音能力）时没有这一项。打开菜单时的候选原文记在 `candidate` 里，点下去时拿它和那一格现在的候选比。
   */
  static readAloudAction(spoken: string | null, candidate: string = ""): ManagementAction | null {
    if (spoken === null || spoken.length === 0) {
      return null;
    }
    return {
      id: "READ_ALOUD",
      title: "朗读",
      announcement: "",
      confirmationRequired: false,
      menuItemId: MENU_ITEM_BASE + 8,
      candidate: candidate,
      spoken: spoken,
    };
  }

  /** 点下「加入生词本」时那一格的候选是否还是打开菜单时的那一个：原文和释义都相同才算。 */
  static samePersonalWordbookSubject(
    opened: ManagementAction,
    current: ManagementAction | null,
  ): boolean {
    return (
      current !== null &&
      opened.id === "ADD_TO_PERSONAL_WORDBOOK" &&
      current.id === "ADD_TO_PERSONAL_WORDBOOK" &&
      opened.candidate === current.candidate &&
      opened.gloss === current.gloss
    );
  }

  /** A 2in1 touch long press: the gloss, then the last group. A candidate without a real translation (an English candidate, say) still gets 朗读 alone when there is something to read, so touch reaches it as right click does. */
  static touchActions(
    gloss: string,
    isTranslation: boolean,
    candidate: string = "",
    spoken: string | null = null,
  ): ManagementAction[] {
    const action: ManagementAction | null = isTranslation
      ? CandidateManagementAction.touchGlossAction(gloss)
      : null;
    if (action === null) {
      const read: ManagementAction | null = CandidateManagementAction.readAloudAction(spoken, candidate);
      return read === null ? [] : [read];
    }
    return [action].concat(
      CandidateManagementAction.lastGroup(gloss, isTranslation, candidate, spoken),
    );
  }

  /** 菜单最后那一组：「朗读」在前，「加入生词本」在后，各自不满足条件时不出。 */
  private static lastGroup(
    gloss: string,
    isTranslation: boolean,
    candidate: string,
    spoken: string | null,
  ): ManagementAction[] {
    const group: ManagementAction[] = [];
    const read: ManagementAction | null = CandidateManagementAction.readAloudAction(
      spoken,
      candidate,
    );
    if (read !== null) {
      group.push(read);
    }
    const personal: ManagementAction | null = CandidateManagementAction.personalWordbookAction(
      gloss,
      isTranslation,
      candidate,
    );
    if (personal !== null) {
      group.push(personal);
    }
    return group;
  }

  /**
   * The full menu the source opens with a right click: the gloss first when the candidate carries a real translation, then 优先显示, the five fixed slots, 取消固定 and 删除词条. A 2in1 opens it with a right click; a phone has no right click, so its long press opens this rather than the gloss alone, as Android's long press does. An Engine annotation such as a Wubi code shares the gloss slot and is not a word, so it is never offered as text.
   */
  static managementActions(
    gloss: string,
    isTranslation: boolean,
    fixedPosition: number,
    available: boolean,
    includeRemove: boolean,
    candidate: string = "",
    spoken: string | null = null,
  ): ManagementAction[] {
    const actions: ManagementAction[] = CandidateManagementAction.actionsForFixedPosition(
      fixedPosition,
      available,
      includeRemove,
    );
    const glossAction: ManagementAction | null = isTranslation
      ? CandidateManagementAction.glossAction(gloss)
      : null;
    const listed: ManagementAction[] =
      glossAction === null ? actions : [glossAction].concat(actions);
    return listed.concat(
      CandidateManagementAction.lastGroup(gloss, isTranslation, candidate, spoken),
    );
  }

  static validatePosition(position: number): number {
    if (position < 1 || position > 5) {
      throw new Error("Candidate position must be between 1 and 5");
    }
    return position;
  }
}
