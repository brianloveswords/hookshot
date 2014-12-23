all: local linux

linux:
	rm -f deployer.linux
	vagrant provision

local:
	cargo build --release

clean:
	rm -f deployer.linux
	rm -rf target
