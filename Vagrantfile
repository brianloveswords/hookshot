# -*- mode: ruby -*-
# vi: set ft=ruby :

VAGRANTFILE_API_VERSION = '2'
Vagrant.configure(VAGRANTFILE_API_VERSION) do |config|
  config.vm.box = 'ubuntu/trusty64'
  config.vm.synced_folder '.', '/mnt/vagrant'
  config.vm.network :private_network, ip: '192.168.30.100'
  config.ssh.forward_agent = true
  config.vm.provision 'shell',
    inline: <<-eos
      echo UPDATING APT CACHE
      apt-get update -y
      echo INSTALLING DEPS
      apt-get install git libssl-dev python-pip pkg-config -y
      pip install ansible
      echo INSTALLING RUST
      curl -sf -L https://static.rust-lang.org/rustup.sh | sh -s -- -y > /dev/null 2>&1
      source $HOME/.cargo/env
      echo UPDATE RUST
      cargo update
      echo BUILD PACKAGE
      cd /mnt/vagrant
      make release
    eos
end
