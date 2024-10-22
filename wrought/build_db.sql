PRAGMA foreign_keys=OFF;
BEGIN TRANSACTION;
CREATE TABLE Events (
                 id integer primary key,
                 group_id integer NOT NULL REFERENCES Groups(id),
                 action_type text NOT NULL,
                 file_path text
             , before_hash text, after_hash text);
INSERT INTO Events VALUES(1,1,'write','foo.txt','x74e2QL7jdTUiZfGRS9dfg','-Xgt15mdwUs5wTKXNebk7w');
INSERT INTO Events VALUES(2,1,'write','bar.txt',NULL,NULL);
INSERT INTO Events VALUES(3,1,'read','zap.txt',NULL,NULL);
INSERT INTO Events VALUES(4,2,'write','foo.txt','x74e2QL7jdTUiZfGRS9dfg','-Xgt15mdwUs5wTKXNebk7w');
CREATE TABLE Groups (
                 id integer primary key
             , command text);
INSERT INTO "Groups" VALUES(1,'init');
INSERT INTO "Groups" VALUES(2,'something');
COMMIT;