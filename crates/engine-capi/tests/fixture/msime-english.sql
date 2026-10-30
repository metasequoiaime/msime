-- 最小的英文词库：建表语句与引擎的 `ensure_english_schema` 相同。每个代次都要求有这个库。
CREATE TABLE english_words(word TEXT COLLATE BINARY NOT NULL, display TEXT NOT NULL, weight INTEGER NOT NULL DEFAULT 0, PRIMARY KEY(word, display)) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS en_zh_glosses(english TEXT COLLATE BINARY PRIMARY KEY, chinese_gloss TEXT NOT NULL) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS zh_en_glosses(chinese TEXT COLLATE BINARY PRIMARY KEY, english_gloss TEXT NOT NULL) WITHOUT ROWID;
INSERT INTO english_words VALUES('hello', 'hello', 1000), ('world', 'world', 900);
PRAGMA user_version=3;
