test:
	@vagrant up --no-provision
	@TEST=yes DISABLE_PLAYBOOK_CHECK=yes vagrant provision

build-all: local-build linux-build

linux-build:
	@vagrant up --no-provision
	@vagrant provision

local-build:
	@cargo build --release

release: test local-build
	@cp target/release/deployer release/deployer.darwin

clean:
	@rm -rf target
	@rm -rf release/*

disinfect: clean
	@vagrant destroy -f

.PHONY: build-all build-linux build-local clean test release disinfect
