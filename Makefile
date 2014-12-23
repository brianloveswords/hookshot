all: local linux

linux:
	vagrant provision

local:
	cargo build --release
