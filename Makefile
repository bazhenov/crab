SEED_PAGE:=http://localhost:8080/page/1

db.sqlite:
	touch $@
	refinery migrate
	cargo run --example=test_server -- add-seed "${SEED_PAGE}" 1

out.csv: db.sqlite
	cargo run --example=test_server -- run-crawler --navigate
	cargo run --example=test_server -- export-csv > $@