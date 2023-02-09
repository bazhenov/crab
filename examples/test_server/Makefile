URL:=http://localhost:8080/page/1

db.sqlite:
	touch $@
	refinery migrate -p ../../migrations
	cargo run --example=test_server -- register "${URL}" 1

out.csv: db.sqlite
	cargo run -- run-crawler --navigate
	cargo run -- export-csv > $@