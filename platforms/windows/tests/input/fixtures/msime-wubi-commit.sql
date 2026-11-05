-- 合成编码与词条，只验证五笔提交归属，不依赖发布词库。
CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);
INSERT INTO wubi86 VALUES('wqaa','合成甲',100),('wqab','合成乙',90),('wqab','合成丙',80);
CREATE TABLE wubi98(key TEXT,value TEXT,weight INTEGER);
INSERT INTO wubi98 SELECT * FROM wubi86;
-- 新世纪五笔表：合成行与 86 相同，另有几个只在这张表里的新世纪码（力 ent、男 ler、为 ytny、上 hgd），选着新世纪时才打得出、查得到，用来确认读的是这张表。设置窗口的界面冒烟（tests/tools/settings_ui.ps1）也拿这份词库查码。
CREATE TABLE wubixsj(key TEXT,value TEXT,weight INTEGER);
INSERT INTO wubixsj SELECT * FROM wubi86;
INSERT INTO wubixsj VALUES('ent','力',100),('ler','男',100),('ytny','为',100),('hgd','上',100);
