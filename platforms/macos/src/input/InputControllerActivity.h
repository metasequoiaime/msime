#pragma once

#import <Foundation/Foundation.h>

// 本进程里获得焦点的输入控制器是否还有没完成的输入：组字中的拼音或普通英文（editing_text）、正在录或正在识别的语音。进程此刻退出的话它们会丢，所以离开电脑时的更新安装要等它们结束（UpdateController.h）。只看组字之后的联想候选不算，丢了也不损失用户打的字。只在主线程调用。
BOOL MSIMEInputControllerHasUnfinishedInput(void);
