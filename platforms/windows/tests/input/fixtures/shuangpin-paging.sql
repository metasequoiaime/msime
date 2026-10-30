-- 仅供双拼翻页后选词的两会话测试使用的合成词条：ni'hao 的十二个整词排在前面，四个单字排在后面（第三页后半和只剩一个的第四页），选单字只消耗前一个音节。
CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_n VALUES('ni','n','拟',90),('ni','n','霓',80),('ni','n','倪',70),('ni','n','伲',60);
CREATE TABLE tbl_1_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_h VALUES('hao','h','好',100),('hao','h','号',90);
CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_2_n VALUES
('ni''hao','nh','拟好',100),
('ni''hao','nh','霓好',99),
('ni''hao','nh','泥好',97),
('ni''hao','nh','妮好',96),
('ni''hao','nh','旎好',95),
('ni''hao','nh','祢好',94),
('ni''hao','nh','铌好',93),
('ni''hao','nh','昵好',92),
('ni''hao','nh','伲好',91),
('ni''hao','nh','鲵好',90),
('ni''hao','nh','怩好',89),
('ni''hao','nh','匿好',88);
