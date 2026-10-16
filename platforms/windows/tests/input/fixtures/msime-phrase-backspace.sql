-- 退格回归只用三个合成词条，不读取用户或发布词库。
CREATE TABLE tbl_1_w(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_w VALUES('wo','w','我',100);
CREATE TABLE tbl_1_s(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_s VALUES('shi','s','是',100);
CREATE TABLE tbl_1_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_h VALUES('hao','h','好',100);
CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);
CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);
CREATE INDEX idx_quick_parases_key_weight ON quick_parases(key,weight DESC);
