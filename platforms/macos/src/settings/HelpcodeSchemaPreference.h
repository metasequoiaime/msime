#pragma once
namespace metasequoia::mac {
inline int NormalizeHelpcodeSchemaPreference(int schema) { return schema >= 0 && schema < 7 ? schema : 0; }
inline const char *HelpcodeSchemaIdentifier(int schema) {
    switch (NormalizeHelpcodeSchemaPreference(schema)) { case 1: return "ziranma"; case 2: return "shouyou2_0"; case 3: return "shouyouplus"; case 4: return "xiaohe"; case 5: return "jiajia"; case 6: return "wubi86"; default: return "lantian"; }
}
}
