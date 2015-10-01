doc: doc-site test
	@cargo doc
	@rm -rf ./doc-site/*
	@cp -r ./target/doc/* doc-site
	@(cd doc-site											&& \
		git pull												&& \
		git add *												&& \
		git commit -am "[build] `date`"	&& \
		git push                           )

doc-site:
	git clone --depth=1 --single-branch -b gh-pages git@github.com:brianloveswords/deployer.git doc-site

test: src/test/test_repo
	cargo test

release:
	cargo build --release

repack-test-repo:
	cd src/test && tar -czf test_repo.tgz test_repo

src/test/test_repo:
	cd src/test && tar -xzf test_repo.tgz

test-hook:
	echo "date: `date`, foo: ${foo}, bar: ${bar}" >> messages.txt

.PHONY: test docs release
