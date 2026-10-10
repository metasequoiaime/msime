#pragma once
#include "resource.h"
#include "../../../../shared/contracts/msime_edition.h"

#define IME_NAME L"MetasequoiaImeTsf"

#define TEXTSERVICE_MODEL L"Apartment"
// 本版本注册的语言（版本表 platforms.windows.langid）：中文版本都是简体中文 0x0804，即 MAKELANGID(LANG_CHINESE, SUBLANG_CHINESE_SIMPLIFIED)；日文版是 0x0411（日语），越南文版是 0x042A（越南语），藏文版是 0x0451（藏语）。TSF 按它把文本服务列在设置的对应语言下，提交的文字也按它标上 GUID_PROP_LANGID。
#define TEXTSERVICE_LANGID MSIME_EDITION_LANGID
#define TEXTSERVICE_ICON_INDEX -IDIS_METASEQUOIAIME
#define TEXTSERVICE_DIC L"MetasequoiaIMESimplifiedQuanPin.txt"
#define TEXTSERVICE_DIC_DB L"cutted_flyciku_with_jp.db"
#define FANYLOGFILE_ "fanydebug.log"
#define FANYLOGFILE L"fanydebug_w.log"

#define IME_MODE_ON_ICON_INDEX IDI_IME_MODE_ON
#define IME_MODE_OFF_ICON_INDEX IDI_IME_MODE_OFF
#define IME_MODE_ON_DARK_ICON_INDEX IDI_IME_MODE_ON_DARK
#define IME_MODE_OFF_DARK_ICON_INDEX IDI_IME_MODE_OFF_DARK
#define IME_MODE_ON_JP_ICON_INDEX IDI_IME_MODE_ON_JP
#define IME_MODE_ON_JP_DARK_ICON_INDEX IDI_IME_MODE_ON_JP_DARK
#define IME_MODE_ON_KR_ICON_INDEX IDI_IME_MODE_ON_KR
#define IME_MODE_ON_KR_DARK_ICON_INDEX IDI_IME_MODE_ON_KR_DARK
#define IME_MODE_ON_SHUANGPIN_ICON_INDEX IDI_IME_MODE_ON_SHUANGPIN
#define IME_MODE_ON_SHUANGPIN_DARK_ICON_INDEX IDI_IME_MODE_ON_SHUANGPIN_DARK
#define IME_MODE_ON_WUBI_ICON_INDEX IDI_IME_MODE_ON_WUBI
#define IME_MODE_ON_WUBI_DARK_ICON_INDEX IDI_IME_MODE_ON_WUBI_DARK
#define IME_MODE_ON_CANTONESE_ICON_INDEX IDI_IME_MODE_ON_CANTONESE
#define IME_MODE_ON_CANTONESE_DARK_ICON_INDEX IDI_IME_MODE_ON_CANTONESE_DARK
#define IME_MODE_ON_ZHUYIN_ICON_INDEX IDI_IME_MODE_ON_ZHUYIN
#define IME_MODE_ON_ZHUYIN_DARK_ICON_INDEX IDI_IME_MODE_ON_ZHUYIN_DARK
#define IME_MODE_ON_VIETNAMESE_ICON_INDEX IDI_IME_MODE_ON_VIETNAMESE
#define IME_MODE_ON_VIETNAMESE_DARK_ICON_INDEX IDI_IME_MODE_ON_VIETNAMESE_DARK
#define IME_MODE_ON_TIBETAN_ICON_INDEX IDI_IME_MODE_ON_TIBETAN
#define IME_MODE_ON_TIBETAN_DARK_ICON_INDEX IDI_IME_MODE_ON_TIBETAN_DARK
#define IME_MODE_ON_STROKE_ICON_INDEX IDI_IME_MODE_ON_STROKE
#define IME_MODE_ON_STROKE_DARK_ICON_INDEX IDI_IME_MODE_ON_STROKE_DARK
#define IME_MODE_CAP_ICON_INDEX IDI_IME_MODE_CAP
#define IME_MODE_CAP_DARK_ICON_INDEX IDI_IME_MODE_CAP_DARK
#define IME_DOUBLE_ON_INDEX IDI_DOUBLE_SINGLE_BYTE_ON
#define IME_DOUBLE_OFF_INDEX IDI_DOUBLE_SINGLE_BYTE_OFF
#define IME_PUNCTUATION_ON_INDEX IDI_PUNCTUATION_ON
#define IME_PUNCTUATION_OFF_INDEX IDI_PUNCTUATION_OFF

#define METASEQUOIAIME_FONT_DEFAULT L"Microsoft YaHei UI"
#define METASEQUOIAIME_LOCALE_DEFAULT L"zh-CN"

//---------------------------------------------------------------------
// defined max pinyin input length
//---------------------------------------------------------------------
#define MAX_PINYIN_LENGTH (64) // Maximum number of characters allowed in pinyin input

//---------------------------------------------------------------------
// defined Candidated Window
//---------------------------------------------------------------------
#define CANDWND_ROW_WIDTH (30)
#define CANDWND_BORDER_COLOR (RGB(0x00, 0x00, 0x00))
#define CANDWND_BORDER_WIDTH (2)
#define CANDWND_NUM_COLOR (RGB(0xB4, 0xB4, 0xB4))
// #define CANDWND_SELECTED_ITEM_COLOR (RGB(0xFF, 0xFF, 0xFF))
// #define CANDWND_SELECTED_BK_COLOR (RGB(0xA6, 0xA6, 0x00))
#define CANDWND_SELECTED_ITEM_COLOR (RGB(0x00, 0x00, 0x00))
#define CANDWND_SELECTED_BK_COLOR (RGB(0xFF, 0xFF, 0xFF))
#define CANDWND_ITEM_COLOR (RGB(0x00, 0x00, 0x00))

//---------------------------------------------------------------------
// defined Candidated List Contants
//---------------------------------------------------------------------
#define CANDWND_ITEM_CNT_PER_PAGE (8) // Do not exceed 9

//---------------------------------------------------------------------
// defined modifier
//---------------------------------------------------------------------
#define _TF_MOD_ON_KEYUP_SHIFT_ONLY (0x00010000 | TF_MOD_ON_KEYUP)
#define _TF_MOD_ON_KEYUP_CONTROL_ONLY (0x00020000 | TF_MOD_ON_KEYUP)
#define _TF_MOD_ON_KEYUP_ALT_ONLY (0x00040000 | TF_MOD_ON_KEYUP)

#define CAND_WIDTH (13) // * tmMaxCharWidth

//---------------------------------------------------------------------
// string length of CLSID
//---------------------------------------------------------------------
#define CLSID_STRLEN (38) // strlen("{xxxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxx}")

//
// Message define
//
// #define WM_SHOW_MAIN_WINDOW
// #define WM_HIDE_MAIN_WINDOW
// #define WM_MOVE_CANDIDATE_WINDOW
// #define WM_SET_PARENT_HWND
