CREATE TABLE pages (
  id INT PRIMARY KEY,
  url TEXT NOT NULL,
  depth INT DEFAULT 0,
  content TEXT NULL
);