#include "../../src/settings/HelpcodeSchemaPreference.h"
#include <cassert>
#include <cstring>
int main(){using namespace metasequoia::mac; assert(NormalizeHelpcodeSchemaPreference(-1)==0); assert(NormalizeHelpcodeSchemaPreference(4)==4); assert(NormalizeHelpcodeSchemaPreference(6)==6); assert(NormalizeHelpcodeSchemaPreference(7)==0); assert(std::strcmp(HelpcodeSchemaIdentifier(0),"lantian")==0); assert(std::strcmp(HelpcodeSchemaIdentifier(2),"shouyou2_0")==0); assert(std::strcmp(HelpcodeSchemaIdentifier(6),"wubi86")==0);}
