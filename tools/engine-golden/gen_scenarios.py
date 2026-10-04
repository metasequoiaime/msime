"""Emit the first batch of scripted golden scenarios. Fixture SQL is copied verbatim from the reference ctests (paths in each scenario's `source`)."""
import json
import os
import sys

OUT = sys.argv[1]
T = "tests/src/"

# ---- fixtures -------------------------------------------------------------

# test_input_session.cpp:390-430 and :534-535 (fixture M), helpcode files :384-388.
M_SQL = ";".join([
    "CREATE TABLE tbl_2_n(key TEXT, jp TEXT, value TEXT, weight INTEGER)",
    "INSERT INTO tbl_2_n VALUES('ni''hao', 'nh', '你好', 200)",
    "INSERT INTO tbl_2_n VALUES('ni''hao', 'nh', '拟好', 100)",
    "CREATE TABLE tbl_2_z(key TEXT, jp TEXT, value TEXT, weight INTEGER)",
    "INSERT INTO tbl_2_z VALUES('zhong''guo', 'zg', '中国', 200)",
    "CREATE TABLE tbl_2_d(key TEXT, jp TEXT, value TEXT, weight INTEGER)",
    "INSERT INTO tbl_2_d VALUES('dong''gua', 'dg', '冬瓜', 200)",
    "INSERT INTO tbl_2_d VALUES('dong''an', 'da', '东安', 200)",
    "CREATE TABLE tbl_2_b(key TEXT, jp TEXT, value TEXT, weight INTEGER)",
    "INSERT INTO tbl_2_b VALUES('bu''hao', 'bh', '不好', 200)",
    "INSERT INTO tbl_2_b VALUES('bu''hao', 'bh', '补好', 100)",
    "INSERT INTO tbl_2_n VALUES('ni''hao', 'nh', '𠀀方案𠮷', 90)",
    "INSERT INTO tbl_2_n VALUES('ni''hao', 'nh', 'C语言 2', 80)",
    "INSERT INTO tbl_2_n VALUES('ni''hao', 'nh', 'GitHub', 70)",
    "CREATE TABLE tbl_1_j(key TEXT, jp TEXT, value TEXT, weight INTEGER)",
    "INSERT INTO tbl_1_j VALUES('ju', 'j', '居', 100)",
    "CREATE TABLE tbl_1_q(key TEXT, jp TEXT, value TEXT, weight INTEGER)",
    "INSERT INTO tbl_1_q VALUES('qu', 'q', '去', 100)",
    "CREATE TABLE tbl_1_x(key TEXT, jp TEXT, value TEXT, weight INTEGER)",
    "INSERT INTO tbl_1_x VALUES('xu', 'x', '需', 100)",
    "CREATE TABLE tbl_1_y(key TEXT, jp TEXT, value TEXT, weight INTEGER)",
    "INSERT INTO tbl_1_y VALUES('yu', 'y', '与', 100)",
    "INSERT INTO tbl_1_x VALUES('xi', 'x', '西', 100)",
    "CREATE TABLE tbl_2_t(key TEXT,jp TEXT,value TEXT,weight INTEGER)",
    "INSERT INTO tbl_2_t VALUES('te''le','tl','特乐',100)",
    "CREATE TABLE tbl_3_x(key TEXT,jp TEXT,value TEXT,weight INTEGER)",
    "CREATE TABLE tbl_1_t(key TEXT,jp TEXT,value TEXT,weight INTEGER)",
    "INSERT INTO tbl_1_t VALUES('te','t','特',100)",
    "CREATE TABLE tbl_1_l(key TEXT,jp TEXT,value TEXT,weight INTEGER)",
    "INSERT INTO tbl_1_l VALUES('le','l','乐',100)",
    "INSERT INTO tbl_1_l VALUES('lve','l','掠',100)",
    "CREATE TABLE tbl_1_h(key TEXT,jp TEXT,value TEXT,weight INTEGER)",
    "INSERT INTO tbl_1_h VALUES('hao','h','好',100)",
    "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER)",
    "INSERT INTO tbl_1_n VALUES('nve','n','虐',100)",
    "CREATE TABLE tbl_4_x(key TEXT,jp TEXT,value TEXT,weight INTEGER)",
    "CREATE TABLE tbl_7_n(key TEXT,jp TEXT,value TEXT,weight INTEGER)",
    "CREATE TABLE tbl_others_n(key TEXT,jp TEXT,value TEXT,weight INTEGER)",
]) + ";"
M_FILES = {
    "helpcodes/helpcode.txt": "你=ab\n拟=cd\n好=ef\n",
    "helpcodes/zrm_helpcode_big_unique.txt": "你=cb\n拟=ad\n好=ef\n",
    "helpcodes/shouyou2_0_helpcode.txt": "你=ab\n拟=cd\n好=ef\n",
    "helpcodes/shouyouplus_helpcode.txt": "你=ab\n拟=cd\n好=ef\n",
    "helpcodes/xiaohe_helpcode.txt": "你=ab\n拟=cd\n好=ef\n",
}
M = {"databases": {"msime-pinyin.db": M_SQL}, "files": M_FILES}
M_SRC = T + "test_input_session.cpp:384-430,534-535"

# test_input_session.cpp:131-144 (FQ), :146-159 (FMX), :161-170 (FSP), :172-181 (FWB).
FQ = {"databases": {"msime-pinyin.db": "BEGIN;"
      "CREATE TABLE tbl_1_n(key TEXT, jp TEXT, value TEXT, weight INTEGER);"
      "INSERT INTO tbl_1_n VALUES('ni', 'n', '甲', 100);"
      "INSERT INTO tbl_1_n VALUES('ni', 'n', '乙', 90);"
      "INSERT INTO tbl_1_n VALUES('ni', 'n', '丙', 80);"
      "INSERT INTO tbl_1_n VALUES('ni', 'n', '丁', 70);"
      "INSERT INTO tbl_1_n VALUES('ni', 'n', '戊', 60);"
      "INSERT INTO tbl_1_n VALUES('ni', 'n', '己', 50);"
      "COMMIT;"}}
FMX = {"databases": {"msime-pinyin.db": "BEGIN;"
       "CREATE TABLE tbl_1_n(key TEXT, jp TEXT, value TEXT, weight INTEGER);"
       "INSERT INTO tbl_1_n VALUES('na', 'n', '甲', 5000000);"
       "INSERT INTO tbl_1_n VALUES('ne', 'n', '乙', 5000000);"
       "INSERT INTO tbl_1_n VALUES('ni', 'n', '丙', 3000000);"
       "COMMIT;"}}
FSP = {"databases": {"msime-pinyin.db": "BEGIN;"
       "CREATE TABLE tbl_2_n(key TEXT, jp TEXT, value TEXT, weight INTEGER);"
       "INSERT INTO tbl_2_n VALUES('ni''hao', 'nh', '你好', 100);"
       "INSERT INTO tbl_2_n VALUES('ni''hao', 'nh', '拟好', 50);"
       "COMMIT;"}}
FWB = {"databases": {"msime-pinyin.db": "BEGIN;"
       "CREATE TABLE wubi86(key TEXT, value TEXT, weight INTEGER);"
       "INSERT INTO wubi86 VALUES('aaaa', '工', 100);"
       "INSERT INTO wubi86 VALUES('aaaa', '或', 50);"
       "COMMIT;"}}

# test_input_session.cpp:1436-1441 (QPH), :1501-1508 (EXP).
QPH = {"databases": {"msime-pinyin.db": "CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);"
       "INSERT INTO quick_parases VALUES('ab','快捷短语一',20);"
       "INSERT INTO quick_parases VALUES('aa','快捷短语二',10);"}}
EXP = {"databases": {"others.db": "CREATE TABLE emoji_pinyin(key TEXT,emoji TEXT,sort_order INTEGER);"
       "INSERT INTO emoji_pinyin VALUES('xiaolian','😀',10);"
       "INSERT INTO emoji_pinyin VALUES('xiao''lian','😄',20);"
       "CREATE TABLE kaomoji(pinyin TEXT,jianpin TEXT,kaomoji TEXT,sort_order INTEGER);"
       "INSERT INTO kaomoji VALUES('haixiu','hx','(*/ω＼*)',10);"}}

# test_temporary_input_session.cpp:86-95.
TMP = {"databases": {"msime-english.db": "CREATE TABLE english_words(word TEXT COLLATE BINARY NOT NULL,display TEXT NOT NULL,"
       "weight INTEGER NOT NULL DEFAULT 0,PRIMARY KEY(word,display)) WITHOUT ROWID;"
       "CREATE TABLE en_zh_glosses(english TEXT PRIMARY KEY,chinese_gloss TEXT NOT NULL);"
       "CREATE TABLE zh_en_glosses(chinese TEXT PRIMARY KEY,english_gloss TEXT NOT NULL);"
       "INSERT INTO english_words VALUES('he','HE',110);"
       "INSERT INTO english_words VALUES('hello','Hello',100);"
       "INSERT INTO english_words VALUES('help','Help',90);"}}
TMP_SRC = T + "test_temporary_input_session.cpp:86-95"

# test_english_input_session.cpp:102-133.
ENG = {"databases": {
    "msime-pinyin.db": "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);"
                "INSERT INTO tbl_1_n VALUES('ni','n','你',200);"
                "INSERT INTO tbl_1_n VALUES('ni','n','倪',100);",
    "msime-english.db": "CREATE TABLE english_words(word TEXT COLLATE BINARY NOT NULL,display TEXT NOT NULL,"
                  "weight INTEGER NOT NULL DEFAULT 0,PRIMARY KEY(word,display)) WITHOUT ROWID;"
                  "CREATE TABLE en_zh_glosses(english TEXT PRIMARY KEY,chinese_gloss TEXT NOT NULL);"
                  "CREATE TABLE zh_en_glosses(chinese TEXT PRIMARY KEY,english_gloss TEXT NOT NULL);"
                  "INSERT INTO english_words VALUES('ni','Ni',300);"
                  "INSERT INTO english_words VALUES('ninja','Ninja',200);"
                  "INSERT INTO english_words VALUES('nimbus','Nimbus',100);"
                  "INSERT INTO english_words VALUES('ni','倪',50);"
                  "INSERT INTO english_words VALUES('hello','Hello',300);"
                  "INSERT INTO english_words VALUES('help','Help',200);"}}
ENG_SRC = T + "test_english_input_session.cpp:102-133"

# test_jianpin_input_session.cpp:84-104.
JP = {"databases": {"msime-pinyin.db": "BEGIN;"
      "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);"
      "INSERT INTO tbl_1_n VALUES('ni','n','你',300);"
      "INSERT INTO tbl_1_n VALUES('na','n','拿',200);"
      "CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);"
      "INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',300);"
      "INSERT INTO tbl_2_n VALUES('ni''hao','nh','拟好',200);"
      "INSERT INTO tbl_2_n VALUES('na''han','nh','呐喊',100);"
      "INSERT INTO tbl_2_n VALUES('ni''shuo','ns','你说',290);"
      "INSERT INTO tbl_2_n VALUES('ni''si','ns','你思',280);"
      "INSERT INTO tbl_2_n VALUES('ni''u','nu','你屋',100);"
      "CREATE TABLE tbl_2_a(key TEXT,jp TEXT,value TEXT,weight INTEGER);"
      "INSERT INTO tbl_2_a VALUES('ai''ni','an','爱你',250);"
      "CREATE TABLE tbl_2_z(key TEXT,jp TEXT,value TEXT,weight INTEGER);"
      "INSERT INTO tbl_2_z VALUES('zhi''chi','zc','知耻',240);"
      "COMMIT;"}}
JP_SRC = T + "test_jianpin_input_session.cpp:84-104"

# test_nine_key_session.cpp:44-68.
NK = {"databases": {
    "msime-pinyin.db": "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);"
                "INSERT INTO tbl_1_n VALUES('ni','n','你',100);"
                "CREATE TABLE tbl_1_m(key TEXT,jp TEXT,value TEXT,weight INTEGER);"
                "INSERT INTO tbl_1_m VALUES('mi','m','米',50);"
                "CREATE TABLE tbl_1_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);"
                "INSERT INTO tbl_1_h VALUES('hao','h','好',100);"
                "CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);"
                "INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',1000);",
    "msime-english.db": "CREATE TABLE english_words(word TEXT,display TEXT,weight INTEGER);"
                  "INSERT INTO english_words VALUES('ok','ok',900);"
                  "INSERT INTO english_words VALUES('old','old',1000);"
                  "INSERT INTO english_words VALUES('older','older',800);"
                  "INSERT INTO english_words VALUES('ogham','ogham',0);"}}
NK_SRC = T + "test_nine_key_session.cpp:44-68"

# test_wubi_input_session.cpp:68-74.
WB = {"english_schema": True, "databases": {"msime-pinyin.db": "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);"
      "INSERT INTO wubi86 VALUES('w','人',20);"
      "INSERT INTO wubi86 VALUES('wq','你',10);"
      "INSERT INTO wubi86 VALUES('wqb','爷',20);"
      "INSERT INTO wubi86 VALUES('wqi','你',10);"
      "INSERT INTO wubi86 VALUES('wqbb','父子',30);"}}
WB_SRC = T + "test_wubi_input_session.cpp:68-74"

# test_wubi_mixed_input_session.cpp:70-79.
WM = {"english_schema": True, "databases": {"msime-pinyin.db":
      "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);"
      "INSERT INTO tbl_1_n VALUES('ni','n','你',10000);"
      "CREATE TABLE tbl_1_z(key TEXT,jp TEXT,value TEXT,weight INTEGER);"
      "INSERT INTO tbl_1_z VALUES('zi','z','子',10000);"
      "CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);"
      "INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',10000),('ni''hao','nh','拟好',9000);"
      "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);"
      "INSERT INTO wubi86 VALUES('wq','你好',10000),('wqaa','众人',9000);"}}
WM_SRC = T + "test_wubi_mixed_input_session.cpp:70-79"

# test_candidate_removal.cpp:77-89.
CR = {"english_schema": True,
      "databases": {
          "msime-pinyin.db": "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);"
                      "INSERT INTO tbl_1_n VALUES('ni','n','你',10000);"
                      "CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);"
                      "INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',10000),('ni''hao','nh','拟好',9000);"
                      "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);"
                      "INSERT INTO wubi86 VALUES('wq','你好',10000),('wq','拟好',9000);",
          "msime-english.db": "INSERT INTO english_words(word,display,weight) VALUES('hello','hello',100),('help','help',50);"},
      "files": {"helpcodes/helpcode.txt": "你=aa\n拟=cc\n"}}
CR_SRC = T + "test_candidate_removal.cpp:77-89"

# test_shuangpin.cpp:129-136.
SP_HC = {"databases": {"msime-pinyin.db": "CREATE TABLE tbl_1_s(key TEXT, jp TEXT, value TEXT, weight INTEGER);"
         "INSERT INTO tbl_1_s VALUES('shi', 's', '使', 200);"
         "INSERT INTO tbl_1_s VALUES('shi', 's', '是', 100);"},
         "files": {"helpcodes/helpcode.txt": "使=ab\n是=uc\n"}}
SP_HC_SRC = T + "test_shuangpin.cpp:129-136"

# test_personal_dictionary.cpp:53-60.
PD = {"english_schema": True, "databases": {"msime-pinyin.db":
      "CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);"
      "CREATE TABLE tbl_7_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);"
      "CREATE TABLE tbl_others_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);"
      "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);"
      "CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);"}}
PD_SRC = T + "test_personal_dictionary.cpp:53-60"

# ---- step helpers ---------------------------------------------------------

def ty(text): return {"op": "type", "arg": text}
def ch(c, shift=False):
    d = {"op": "char", "arg": c}
    if shift:
        d["shift"] = True
    return d
def sh(c): return ch(c, True)
def cmd(name): return {"op": "command", "arg": name}
def sel(i): return {"op": "select", "arg": i}
def selw(w): return {"op": "select", "arg": {"word": w}}
def punct(c): return {"op": "punctuation", "arg": c}
def op(name, arg=None, **extra):
    d = {"op": name}
    if arg is not None:
        d["arg"] = arg
    d.update(extra)
    return d
def q(db, sql): return {"op": "query", "db": db, "arg": sql}
CANCEL = cmd("Cancel")
REOPEN = op("reopen")
JOURNAL = op("dump_journal")

scenarios = []
def add(name, source, fixture, options, steps, note=None, covers=None):
    s = {"name": name, "source": source}
    if note:
        s["note"] = note
    if covers:
        s["covers"] = covers
    s["fixture"] = fixture
    s["options"] = options
    s["steps"] = steps
    scenarios.append(s)

# InputSession(Quanpin, autocorrect=1, helpcode=false) as in test_input_session.cpp:432.
PORT = {"autocorrect_types": 1, "helpcode": False}

# ---- quanpin basics (test_input_session.cpp) ------------------------------
add("qp_portable_selection", M_SRC + "; cases :432-446", M, PORT,
    [ty("xi'te'le"), selw("西"), selw("特乐"), q("msime-pinyin.db", "SELECT key,value FROM tbl_3_x ORDER BY 1,2"),
     ty("xi'te'le"), selw("西"), CANCEL], covers=["partial commit", "phrase learning", "cancel"])
add("qp_backspace_abandoned_phrase", M_SRC + "; case :448-459", M, PORT,
    [ty("xi'te'le"), selw("西")] + [cmd("Backspace")] * 6 + [ty("nihao"), selw("你好"),
     q("msime-pinyin.db", "SELECT key,value FROM tbl_3_x ORDER BY 1,2")], covers=["backspace", "partial commit"])
add("qp_punctuation_finishes_composition", M_SRC + "; case :461-468", M,
    {"autocorrect_types": 1, "helpcode": False, "learning": False},
    [ty("xi'te'le"), selw("西"), punct(",")], covers=["punctuation", "partial commit"])
add("qp_generated_sentence", M_SRC + "; case :507-530", M, PORT,
    [ty("xi'te'le'hao"), selw("西"), sel(0), q("msime-pinyin.db", "SELECT key,value FROM tbl_4_x ORDER BY 1,2")],
    covers=["lattice sentence", "phrase learning"])
add("qp_umlaut_v_syllables", M_SRC + "; case :624-637", M, {},
    [ty("jv"), CANCEL, ty("qv"), CANCEL, ty("xv"), CANCEL, ty("yv")], covers=["quanpin basics"])
add("qp_lue_nue_alias", M_SRC + "; case :639-651", M, {}, [ty("lue"), CANCEL, ty("nue")], covers=["quanpin basics"])
add("qp_missing_final_g", M_SRC + "; case :653-667", M, {},
    [ty("zhonguo"), sel(0), ty("zhon'guo"), sel(0), ty("zhongguo"), CANCEL, ty("dongua"), CANCEL,
     ty("donggua"), CANCEL, ty("dongan")], covers=["quanpin basics"])
add("qp_helpcode_disabled_uppercase", M_SRC + "; case :676-680", M, {"autocorrect_types": 1, "helpcode": False},
    [ty("ni"), ch("H")], covers=["helpcode"])
add("qp_uppercase_and_duplicate_apostrophe", M_SRC + "; cases :805-819", M, {},
    [ch("N"), ty("ni"), ch("H"), CANCEL, ty("ni'"), ch("'")], covers=["quanpin basics"])
add("qp_nihao_select_and_commit", M_SRC + "; cases :821-849", M, {},
    [ty("nihao"), sel(1), ty("nihao"), selw("你好"), ty("nihao"), sel(99), cmd("CommitCandidate"),
     ty("nihao"), punct(",")], covers=["quanpin basics", "selection", "punctuation"])
add("qp_candidate_digit_key", M_SRC + "; case :887-889", M, {},
    [ty("nihao"), op("candidate_key", "2")], covers=["selection"])
add("qp_learning_promotes_selection", M_SRC + "; case :891-899", M, {},
    [ty("nihao"), sel(1), JOURNAL, REOPEN, ty("nihao")], covers=["learning"])
add("qp_learning_disabled", M_SRC + "; case :790-803", M, {"learning": False},
    [ty("buhao"), sel(1), JOURNAL, REOPEN, ty("buhao")], covers=["learning"])
add("qp_select_edge", M_SRC + "; case :901-940", M, {"learning": False},
    [ty("nihao"), op("select_edge", {"word": "拟好"}, edge="first"),
     ty("nihao"), op("select_edge", {"word": "拟好"}, edge="last"),
     ty("nihao"), op("select_edge", {"word": "𠀀方案𠮷"}, edge="first"),
     ty("nihao"), op("select_edge", {"word": "𠀀方案𠮷"}, edge="last"),
     ty("nihao"), op("select_edge", {"word": "C语言 2"}, edge="first"),
     ty("nihao"), op("select_edge", {"word": "C语言 2"}, edge="last"),
     ty("nihao"), op("select_edge", {"word": "GitHub"}, edge="first")], covers=["selection"])
add("qp_backspace_raw_cancel_switch", M_SRC + "; cases :942-962", M, {},
    [ty("nihao"), cmd("Backspace"), cmd("CommitRaw"), ty("nihao"), CANCEL, ty("nihao"),
     op("switch_scheme", "shuangpin")], covers=["backspace", "commit raw", "cancel"])
add("qp_helpcode_filter", M_SRC + "; cases :991-1067", M, {},
    [ty("nihao"), ch("C"), CANCEL, ty("nih"), ch("C"), CANCEL, op("set_helpcode_enabled", False), ty("nihao"),
     ch("C"), CANCEL, op("set_helpcode_enabled", True), op("set_helpcode_schema", "ziranma"), ty("nihao"), ch("A")],
    covers=["helpcode"])
add("sp_helpcode_filter", M_SRC + "; cases :991-1067", M, {"scheme": "shuangpin"},
    [ty("nihcc"), CANCEL, ty("nihc"), ch("A"), ch("B"), CANCEL, ty("nihc"), ch("A"), ch("E")],
    covers=["shuangpin", "helpcode"])
add("qp_idle_passthrough", M_SRC + "; case :1069-1077", M, {},
    [ch("1"), cmd("Backspace"), cmd("CommitRaw"), sel(0), ch("'")], covers=["idle keys"])
add("qp_caret_editing", M_SRC + "; segment boundaries :1289-1324 plus caret commands", M, {},
    [ty("nihaoma"), cmd("MoveLeft"), cmd("MoveLeft"), cmd("MoveHome"), cmd("MoveRight"), cmd("DeleteForward"),
     cmd("MoveEnd"), cmd("Backspace"), CANCEL, ty("ni'hao")], covers=["caret", "segmentation"])
add("sp_segment_boundaries", M_SRC + "; case :1289-1324", M, {"scheme": "shuangpin"},
    [ty("nihaoma")], covers=["shuangpin", "segmentation"])
add("qp_idle_chinese_punctuation", T + "test_input_session.cpp:850-885 (idle punctuation map)", M, {},
    [punct(c) for c in ".\"\"''()[]`$^_<<>>\\"] + [op("set_chinese_punctuation_enabled", False), punct(".")],
    covers=["punctuation"])
add("qp_paired_punctuation_disabled", T + "test_input_session.cpp:863-885; test_runtime_isolation.cpp:172-202", M,
    {"paired_punctuation": False}, [punct(c) for c in "\"\"''<<>>"] + [op("set_paired_punctuation_enabled", True),
    punct("\""), ty("ni"), punct("\""), op("set_punctuation_lock", 2), punct(","), op("set_punctuation_lock", 0),
    punct(",")], covers=["punctuation"])

# ---- local modes ----------------------------------------------------------
add("local_unicode_mode", M_SRC + "; cases :1263-1386", M, {},
    [sh("U"), ty("4e00"), sel(0), sh("U"), ty("1f600"), sel(0), sh("U"), ty("d800"), CANCEL, sh("U"),
     ty("110000"), CANCEL, sh("U"), ch("0"), cmd("Backspace"), cmd("Backspace")], covers=["local modes"])
add("local_quick_phrase", T + "test_input_session.cpp:1436-1470", QPH, {},
    [sh("K"), ch("a"), ch("b"), sel(0), sh("K"), ch("a"), ch("9"), CANCEL], covers=["local modes"])
add("local_emoji_kaomoji", T + "test_input_session.cpp:1501-1595", EXP, {},
    [sh("E"), ty("xiaolian"), sel(0), sh("E"), ty("xiao'lian"), CANCEL, op("switch_scheme", "shuangpin"),
     sh("M"), ty("hx"), sel(0)], covers=["local modes"])
add("local_date_time_unmatched", T + "test_temporary_input_session.cpp:246-259", TMP, {},
    [sh("T"), ty("xin"), cmd("CommitCandidate")],
    note="Only the clock-independent part of date/time mode: the public Session exposes no date/time provider (InputSession::set_local_date_time_provider, core/input_session.h:109), so rq/sj/xq candidates are not recorded.",
    covers=["local modes"])
add("local_super_jianpin", JP_SRC + "; case :185-213", JP, {},
    [sh("J"), ty("nh"), selw("你好"), sh("J"), cmd("Backspace")], covers=["local modes", "jianpin"])
add("local_super_jianpin_learning", JP_SRC + "; case :217-228", JP, {"frequency": {"mode": "pin"}},
    [sh("J"), ty("nh"), selw("拟好"), REOPEN, sh("J"), ty("nh"), JOURNAL], covers=["learning", "jianpin"])
add("local_modes_disabled", T + "test_temporary_input_session.cpp:261-271; test_jianpin_input_session.cpp:197-203", JP,
    {"local_modes": {"temporary_english": False, "temporary_japanese": False, "super_jianpin": False}},
    [sh("Y"), sh("R"), sh("J")], covers=["local modes"])

# ---- temporary English / Japanese -----------------------------------------
add("temp_english_mode", TMP_SRC + "; case :96-141", TMP, {},
    [sh("Y"), punct(","), sh("Y"), ty("he"), sel(2), sh("Y"), cmd("Backspace"), sh("Y"), cmd("CommitCandidate"),
     sh("Y"), ty("he"), CANCEL, sh("Y"), op("switch_scheme", "shuangpin")], covers=["temporary english"])
add("temp_english_enter_and_learning", TMP_SRC + "; case :143-166", TMP, {"frequency": {"mode": "promote"}},
    [sh("Y"), ty("he"), cmd("CommitRaw"), sh("Y"), ty("he"), selw("Help"), JOURNAL, REOPEN, sh("Y"), ty("he")],
    covers=["temporary english", "learning"])
add("temp_japanese_mode", TMP_SRC + "; case :168-244", TMP, {},
    [sh("R"), punct(","), sh("R"), ty("ka"), selw("か"), sh("R"), cmd("Backspace"), sh("R"), cmd("CommitCandidate"),
     sh("R"), op("finish"), sh("R"), ch("k"), cmd("Backspace"), CANCEL, sh("R"), ty("ka"), CANCEL, sh("R"),
     op("switch_scheme", "shuangpin"), sh("R"), ty("ka"), selw("か")], covers=["temporary japanese"])
add("temp_japanese_enter", TMP_SRC + "; case :230-244", TMP, {}, [sh("R"), ty("ka"), cmd("CommitRaw")],
    covers=["temporary japanese"])

# ---- English --------------------------------------------------------------
add("english_mixed_candidates", ENG_SRC + "; case :136-148", ENG,
    {"english": {"mixed_candidates": True, "minimum_prefix": 2}}, [ty("ni")], covers=["english"])
add("english_minimum_prefix", ENG_SRC + "; case :150-158", ENG,
    {"english": {"mixed_candidates": True, "minimum_prefix": 3}}, [ty("ni"), ch("n")], covers=["english"])
add("english_dedicated_mode", ENG_SRC + "; case :167-201", ENG, {},
    [op("set_dedicated_english", True), ty("HE"), sel(1), ty("Codex"), cmd("CommitRaw"),
     q("msime-english.db", "SELECT word,display,weight FROM english_words WHERE word='codex'"), JOURNAL],
    covers=["english", "learning"])

# ---- frequency learning (test_input_session.cpp:1083-1245) -----------------
for mode, step in (("disabled", 1), ("pin", 1), ("halve", 1), ("linear", 2), ("promote", 1)):
    add("freq_mode_" + mode, T + "test_input_session.cpp:131-144,1086-1120", FQ,
        {"frequency": {"mode": mode, "trigger_count": 1, "linear_step": step}},
        [ty("ni"), selw("己"), REOPEN, ty("ni"), JOURNAL], covers=["learning"])
add("freq_trigger_count", T + "test_input_session.cpp:1122-1139", FQ,
    {"frequency": {"mode": "pin", "trigger_count": 2}},
    [ty("ni"), selw("己"), REOPEN, ty("ni"), CANCEL, ty("ni"), selw("己"), REOPEN, ty("ni"), JOURNAL],
    covers=["learning"])
add("freq_leading_pick_no_state", T + "test_input_session.cpp:1141-1151", FQ, {"frequency": {"mode": "pin"}},
    [ty("ni"), sel(0), JOURNAL], covers=["learning"])
add("freq_mixed_key_promote", T + "test_input_session.cpp:146-159,1153-1172", FMX, {"frequency": {"mode": "promote"}},
    [ty("n"), selw("丙"), REOPEN, ty("n"), JOURNAL], covers=["learning"])
add("freq_shuangpin", T + "test_input_session.cpp:161-170,1174-1188", FSP,
    {"scheme": "shuangpin", "frequency": {"mode": "pin"}}, [ty("nihc"), selw("拟好"), REOPEN, ty("nihc"), JOURNAL],
    covers=["learning", "shuangpin"])
add("freq_wubi", T + "test_input_session.cpp:172-181,1189-1202", FWB, {"scheme": "wubi", "frequency": {"mode": "pin"}},
    [ty("aaaa"), selw("或"), REOPEN, ty("aaaa"), JOURNAL], covers=["learning", "wubi"])

# ---- shuangpin --------------------------------------------------------------
add("sp_xiaohe_trailing_helpcode", SP_HC_SRC + "; cases :137-158", SP_HC, {"scheme": "shuangpin"},
    [ty("uiu"), CANCEL, ty("ui'u")], covers=["shuangpin", "helpcode"])
add("sp_ziranma_nihao", T + "test_input_session.cpp:161-170 fixture, ziranma profile", FSP,
    {"scheme": "shuangpin", "shuangpin_profile": "ziranma"}, [ty("nihk"), CANCEL, ty("nihc")], covers=["shuangpin"])
add("sp_microsoft_semicolon", T + "test_runtime_isolation.cpp:203-253 (keys only; fixture FSP)", FSP,
    {"scheme": "shuangpin", "shuangpin_profile": "microsoft"}, [ty("b;"), CANCEL, ty("nihkb;"), CANCEL, ty("nihcb;")],
    covers=["shuangpin"])
add("sp_preedit_segmented", T + "test_fuzzy_pinyin.cpp:174-264 (shuangpin_preedit_uses_raw off; fixture FSP)", FSP,
    {"scheme": "shuangpin", "shuangpin_preedit_uses_raw": False}, [ty("nihc"), sel(0)], covers=["shuangpin"])

# ---- wubi -------------------------------------------------------------------
WB_NOTE = "Reference ctest wubi_input_session fails against this build: overlay apply_engine_wubi_prefix_learning.py orders exact code first then weight, and drops the word dedup, so wq gives [你, 父子, 爷, 你]. The overlay behaviour is the product intent and is what is recorded."
add("wubi_prefix_codes", WB_SRC + "; cases :100-138", WB, {"scheme": "wubi"},
    [ty("wq"), CANCEL, ty("w"), CANCEL, ty("wqbb"), CANCEL, ty("wx")], note=WB_NOTE, covers=["wubi"])
add("wubi_select_and_gates", WB_SRC + "; test_input_session.cpp:775-782 gates", WB, {"scheme": "wubi"},
    [ty("wqbb"), sel(0), ch("z"), ty("wqbb"), ch("b"), ch("'"), CANCEL], covers=["wubi"])
WM_NOTE = "Reference ctest wubi_mixed_input_session fails against this build: overlay apply_engine_wubi_mixed_candidates.py appends quanpin rows after wubi rows and recomputes the table answer per keystroke, so a tail after selection can get wubi rows too. The overlay behaviour is the product intent and is what is recorded."
add("wubi_mixed_unmatched_code", WM_SRC + "; case :106-116", WM, {"scheme": "wubi", "wubi_mixed_pinyin": True},
    [ty("nihao"), cmd("Backspace")], note=WM_NOTE, covers=["wubi"])
add("wubi_mixed_reference_quanpin", WM_SRC + "; case :106-116 (quanpin reference session)", WM, {},
    [ty("nihao")], covers=["wubi"])
add("wubi_mixed_matched_code", WM_SRC + "; cases :121-162", WM, {"scheme": "wubi", "wubi_mixed_pinyin": True},
    [ty("wq"), CANCEL, ty("wqaa"), ch("a"), CANCEL, op("set_wubi_mixed_pinyin", False), ty("zi"), CANCEL,
     op("set_wubi_mixed_pinyin", True), ty("zi")], note=WM_NOTE, covers=["wubi"])
add("wubi_mixed_backspace_resets_fallback", WM_SRC + "; case :171-191", WM,
    {"scheme": "wubi", "wubi_mixed_pinyin": True}, [ty("nihao")] + [cmd("Backspace")] * 5 + [ty("wq"), ty("aa")],
    note=WM_NOTE, covers=["wubi", "backspace"])
add("wubi_mixed_tail_after_selection", WM_SRC + "; case :197-212", WM, {"scheme": "wubi", "wubi_mixed_pinyin": True},
    [ty("nihaowq"), selw("你好")], note=WM_NOTE, covers=["wubi", "partial commit"])
add("wubi_mixed_tail_quanpin_reference", WM_SRC + "; case :197-212 (quanpin reference session)", WM, {},
    [ty("nihaowq"), selw("你好")], covers=["partial commit"])

# ---- nine-key ---------------------------------------------------------------
NK_NOTE = "Reference ctest nine_key_session fails against this build (test_nine_key_session.cpp:91-107): overlay apply_engine_lattice_reading.py runs lattice decoding at 2+ syllables, so 64426 yields a Generated 米好 covering all digits ahead of the 2-digit dictionary rows. The overlay behaviour is the product intent and is what is recorded."
add("nine_key_session", NK_SRC + "; cases :75-160", NK, {"learning": False, "english": {"mixed_candidates": True}},
    [ch("6"), op("set_nine_key_enabled", True), ch("0"), ch("1"), ty("65"), CANCEL, ty("64426"), CANCEL, ty("64"),
     sel(999), op("choose_nine_key_spelling", 999), op("choose_nine_key_spelling", {"spelling": "ni"}), ty("426"),
     selw("你好"), ty("64426"), op("choose_nine_key_spelling", {"spelling": "ni"}), op("choose_nine_key_spelling", 0),
     selw("你"), op("finish"), ty("64"), cmd("Backspace"), cmd("Backspace"), cmd("Backspace"), ty("64426"),
     punct(","), ty("64"), cmd("CommitRaw"), ty("64"), CANCEL, ty("64"), op("switch_scheme", "shuangpin"), ch("6"),
     op("switch_scheme", "quanpin"), op("set_nine_key_enabled", False), ch("n"), CANCEL,
     op("set_nine_key_enabled", True), ty("7" * 32), ch("7"), CANCEL], note=NK_NOTE, covers=["nine key"])
add("nine_key_english", NK_SRC + "; case :228-258", NK, {"learning": False, "english": {"mixed_candidates": True}},
    [op("set_nine_key_enabled", True), op("set_dedicated_english", True), ty("65"), CANCEL, ty("653"), CANCEL,
     op("set_dedicated_english", False), ty("65")], note=NK_NOTE, covers=["nine key", "english"])

# ---- pin / remove / fixed position (test_candidate_removal.cpp) -----------
for scheme, key in (("quanpin", "nihao"), ("shuangpin", "nihc"), ("wubi", "wq")):
    for pin_first in (False, True):
        steps = [op("remove", 0), ty(key), op("remove", 99)]
        if pin_first:
            steps.append(op("pin", {"word": "拟好"}))
        steps += [op("remove", {"word": "拟好"}), JOURNAL, op("new_generation", "v2"), ty(key), CANCEL,
                  op("new_generation", "v1"), ty(key)]
        add("remove_" + scheme + ("_after_pin" if pin_first else ""), CR_SRC + "; cases :90-129", CR,
            {"scheme": scheme, "learning": False}, steps, covers=["removal", "learning"])
add("fixed_position_quanpin", T + "test_runtime_isolation.cpp:477-528 (keys only; fixture from " + CR_SRC + ")", CR,
    {"learning": False},
    [ty("nihao"), op("fix_position", {"word": "拟好"}, position=1), op("fix_position", {"word": "拟好"}, position=6),
     JOURNAL, REOPEN, ty("nihao"), op("clear_position", {"word": "拟好"}), JOURNAL], covers=["fixed position"])

# ---- personal dictionary ------------------------------------------------
PD_NOTE = "Reference ctest personal_dictionary fails against this build (test_personal_dictionary.cpp:67-76): overlay apply_engine_english_display.py lets an English code differ from its display, so {English, hello, different} is accepted. Recorded as-is. The invalid-UTF-8 and kind-99 entries of that loop cannot be expressed in JSON and are left to a Rust unit test."
invalid = [("pinyin", "nihao", "你好"), ("pinyin", "ni''hao", "你好"), ("pinyin", "ni'hao", "你"),
           ("wubi", "abcde", "词"), ("quick_phrase", "bad;code", "text"), ("english", "hello", "different"),
           ("english", "dont", "don't")]
steps = [op("validate_entry", {"kind": "pinyin", "key": "NI HAO", "value": "拟好", "weight": 12345})]
steps += [op("validate_entry", {"kind": k, "key": key, "value": v}) for k, key, v in invalid]
steps += [op("dict_edit", {"replacement": {"kind": "pinyin", "key": "ni'hao", "value": "拟好", "weight": 12345}}),
          op("dict_edit", {"replacement": {"kind": "wubi", "key": "wq", "value": "拟好", "weight": 12345}}),
          op("dict_edit", {"replacement": {"kind": "english", "key": "metasequoia", "value": "Metasequoia", "weight": 12345}}),
          op("dict_edit", {"replacement": {"kind": "quick_phrase", "key": "test1", "value": "fixture\nsecond line", "weight": 12345}}),
          op("dict_list", {}), REOPEN, ty("nihao"), CANCEL, op("switch_scheme", "shuangpin"), ty("nihc"), CANCEL,
          op("switch_scheme", "wubi"), ty("wq"), CANCEL, op("switch_scheme", "quanpin"),
          op("set_nine_key_enabled", True), ty("64426"), CANCEL, op("set_nine_key_enabled", False),
          op("set_dedicated_english", True), ty("metasequoia"), CANCEL, op("set_dedicated_english", False),
          sh("K"), ty("test1"), CANCEL, JOURNAL]
add("personal_dictionary_entries", PD_SRC + "; cases :61-100", PD, {}, steps, note=PD_NOTE,
    covers=["personal dictionary"])

# ---- online candidates ------------------------------------------------------
add("online_cloud_and_ai_slots", T + "test_online_input_session.cpp:140-191 (keys only; fixture from " + ENG_SRC + ")", ENG,
    {"learning": False},
    [ty("ni"), op("apply_online_candidates", ["妮"], source="cloud"), op("apply_online_candidates", ["尼"], source="ai"),
     op("apply_online_candidates", ["你"], source="cloud")], covers=["online"])

# ---- paging (host-side view) and session toggles ----------------------------
add("qp_paging_and_page_selection", M_SRC + " (keys only; paging is host-side, recorded as a view)", M,
    {"page_size": 2, "learning": False},
    [ty("nihao"), op("page", "next"), op("select_on_page", 0), ty("nihao"), op("page", "next"), op("page", "next"),
     op("select_on_page", 0), ty("xi'te'le"), op("page", "next"), op("page", "prev"), op("select_on_page", 1),
     op("select_on_page", 0)], covers=["paging", "selection", "partial commit"])
add("qp_initial_expansion_and_toggles", JP_SRC + " (keys only)", JP, {"learning": True, "frequency": {"mode": "promote"}},
    [ty("n"), op("expand_initial_candidates"), op("reset_cache"), CANCEL, op("set_personal_context_enabled", False),
     ty("ni"), sel(0), ty("aini"), sel(0), JOURNAL, ty("ni"), op("set_personal_context_enabled", True), op("reset_context"),
     punct("<"), op("balance_paired_punctuation_after_auto_close", "<"), punct("<"), punct(">")],
    covers=["paging", "learning", "punctuation"])

os.makedirs(OUT, exist_ok=True)
names = set()
for s in scenarios:
    assert s["name"] not in names, s["name"]
    names.add(s["name"])
    with open(os.path.join(OUT, s["name"] + ".json"), "w", encoding="utf-8") as f:
        json.dump(s, f, ensure_ascii=False, indent=1)
        f.write("\n")
print(len(scenarios))
