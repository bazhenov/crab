ALTER TABLE pages ADD compressed INT;
ALTER TABLE pages ADD _content BLOB;
UPDATE pages SET _content = CAST(content AS BLOB);
ALTER TABLE pages DROP content;
ALTER TABLE pages RENAME _content TO content;
