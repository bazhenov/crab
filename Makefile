SEED_PAGE:=http://localhost:8080/page/1

db.sqlite:
	touch $@
	refinery migrate
	cargo run -- add-seed "${SEED_PAGE}"

out.csv: db.sqlite
	cargo run -- run-crawler --navigate
	cargo run -- export-csv > $@