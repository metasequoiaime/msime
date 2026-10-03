#ifndef Edition
#define Edition "full"
#endif
#if Edition == "full"
#define MyEditionIsFull 1
#define MyEditionAppName "Metasequoia IME 水杉输入法"
#define MyEditionAppId "{{A7C3E91F-4B2D-4E8A-9F1C-6D5E8B0A2C4D}"
#define MyEditionClsid "{E3062E9A-D834-4637-8958-ED8CFA427D01}"
#define MyEditionInstallDir "metasequoiaime"
#define MyEditionRegistryKey "Software\Metasequoia\MetasequoiaIME"
#define MyEditionWatchdogTask "Metasequoia IME Watchdog"
#define MyEditionInstallerBaseName "MetasequoiaIME_Setup"
#elif Edition == "pinyin"
#define MyEditionIsFull 0
#define MyEditionAppName "Metasequoia IME 水杉拼音"
#define MyEditionAppId "{{EB4125FA-5687-4698-9E43-2C4CA4652132}"
#define MyEditionClsid "{7C927972-6D06-44C7-9B7C-B0D00C77D7C7}"
#define MyEditionInstallDir "metasequoiaime-pinyin"
#define MyEditionRegistryKey "Software\Metasequoia\MetasequoiaIME-Pinyin"
#define MyEditionWatchdogTask "Metasequoia IME Watchdog (Pinyin)"
#define MyEditionInstallerBaseName "MetasequoiaIME-Pinyin_Setup"
#elif Edition == "wubi"
#define MyEditionIsFull 0
#define MyEditionAppName "Metasequoia IME 水杉五笔"
#define MyEditionAppId "{{FF5943BF-0F4B-4999-8504-15235E4516C6}"
#define MyEditionClsid "{FD41CA20-BA49-41FE-96BE-923DCCA191DA}"
#define MyEditionInstallDir "metasequoiaime-wubi"
#define MyEditionRegistryKey "Software\Metasequoia\MetasequoiaIME-Wubi"
#define MyEditionWatchdogTask "Metasequoia IME Watchdog (Wubi)"
#define MyEditionInstallerBaseName "MetasequoiaIME-Wubi_Setup"
#else
#error Unknown edition; use /DEdition= one of full, pinyin, wubi
#endif
