doc: doc-site test
	@cargo doc
	@cp -r target/doc/* doc-site
	@(cd doc-site											&& \
		git pull												&& \
		git add *												&& \
		git commit -am "[build] `date`"	&& \
		git push                           )

doc-site:
	git clone --depth=1 --single-branch -b gh-pages git@github.com:brianloveswords/deployer.git doc-site

test: src/test/test_repo
	cargo test

src/test/test_repo:
	cd src/test && tar -xzf test_repo.tgz

.PHONY: test docs
