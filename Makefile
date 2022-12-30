db.sqlite:
	touch $@
	refinery migrate
	cargo run -- add-seed http://localhost:8080/page/1