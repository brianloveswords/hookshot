test:
	@vagrant up --no-provision
	@TEST=yes vagrant provision

build-all: local-build linux-build

linux-build:
	@vagrant up --no-provision
	@vagrant provision

local-build:
	@cargo build --release

clean:
	@rm -rf target
	@vagrant destroy -f

.PHONY: build-all build-linux build-local clean test
