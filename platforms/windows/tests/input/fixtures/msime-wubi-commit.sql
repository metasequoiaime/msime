-- 合成编码与词条，只验证五笔提交归属，不依赖发布词库。
CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);
INSERT INTO wubi86 VALUES('wqaa','合成甲',100),('wqab','合成乙',90),('wqab','合成丙',80);
CREATE TABLE wubi98(key TEXT,value TEXT,weight INTEGER);
INSERT INTO wubi98 SELECT * FROM wubi86;
