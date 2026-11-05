import * as doc from "./document-style";
import { useConfirm } from "../core/confirm";
import { LinkRow, Row } from "../core/platform-controls";
import { ActionButton } from "./action-button";

/** 粤语双拼键位方案设计者的方案仓库，致谢里的「查看方案仓库」打开它。 */
export const cantoneseShuangpinSchemeRepository =
  "https://github.com/swithinchan/rime-cantonese-shuangpin";

/** 致谢正文，与 iOS、Android 随包的 `resources/licenses/cantonese-shuangpin-scheme-NOTICE.txt` 说的是同一件事；仓库地址由按钮打开，不在正文里重复。 */
export const cantoneseShuangpinCreditText =
  "粤语双拼的键位方案由陳澍鋒（swithinchan）设计，即他的「粵語雙拼（自然碼兼容版）」，经他在 #9799 提议收入水杉输入法。水杉输入法的粤语双拼按同一套键位独立编写，不含方案仓库里的文件。";

/**
 * 「关于 › 开源许可」旁对粤语双拼键位方案设计者陳澍鋒的致谢（#9799）。署名属于致谢和许可，不放在选方案的面板里，免得打扰选择。点开是一段说明，「查看方案仓库」打开他的方案仓库。
 *
 * `harmony` 照鸿蒙「法律信息」一组的写法画成带「查看」按钮的行，其余宿主是「许可与隐私」里的跳转行。
 */
export function CantoneseShuangpinCreditRow({
  harmony,
  openExternalUrl,
}: {
  harmony: boolean;
  openExternalUrl: (url: string) => void | Promise<void>;
}) {
  const { confirm, confirmation } = useConfirm();
  const show = async () => {
    const open = await confirm({
      title: "粤语双拼键位方案",
      message: cantoneseShuangpinCreditText,
      confirmLabel: "查看方案仓库",
      cancelLabel: "关闭",
    });
    if (open) await openExternalUrl(cantoneseShuangpinSchemeRepository);
  };
  return (
    <>
      {harmony ? (
        <Row title="粤语双拼键位方案" detail="陳澍鋒设计">
          <ActionButton
            action={show}
            className={doc.rowButton}
            ariaLabel="查看粤语双拼键位方案的致谢"
            label="查看"
          />
        </Row>
      ) : (
        <LinkRow title="粤语双拼键位方案" value="陳澍鋒设计" onClick={() => void show()} />
      )}
      {confirmation}
    </>
  );
}
