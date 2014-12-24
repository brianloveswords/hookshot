test:
	@vagrant up --no-provision
	@TEST=yes vagrant provision

build-all: local-build linux-build

linux-build:
	@vagrant up --no-provision
	@vagrant provision

local-build:
	@cargo build --release

release: build-all
	@cp deploy/ansible/roles/install/files/deployer.linux release/deployer.linux
	@cp target/release/deployer release/deployer.darwin

clean:
	@rm -rf target
	@rm -rf release/*

disinfect: clean
	@vagrant destroy

.PHONY: build-all build-linux build-local clean test release disinfect
