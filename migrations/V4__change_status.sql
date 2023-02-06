ALTER TABLE pages ADD _status INT CHECK (_status IN (1, 2)) DEFAULT 1;
UPDATE pages SET _status = status;
ALTER TABLE pages DROP status;
ALTER TABLE pages RENAME _status TO status;
