db.sqlite:
	touch $@
	refinery migrate
	cargo run -- add-seed http://localhost:8080/page/1

out.csv: db.sqlite
	cargo run -- run-crawler --navigate
	cargo run -- export-csv > $@