# -*- mode: ruby -*-
# vi: set ft=ruby :

# Vagrantfile API/syntax version. Don't touch unless you know what you're doing!
VAGRANTFILE_API_VERSION = "2"

Vagrant.configure(VAGRANTFILE_API_VERSION) do |config|
  config.vm.box = "ubuntu/trusty64"
  config.vm.network "forwarded_port", guest: 1469, host: 4169
  config.vm.network "private_network", ip: "192.168.100.128"

  config.vm.synced_folder ".", "/mnt/tcp_listener", type: "rsync",
    rsync__exclude: [".git/", "target/"]

  config.ssh.forward_agent = true

  config.vm.provider "virtualbox" do |vm|
    vm.memory = 1024
  end

  config.vm.provision "ansible" do |ansible|
    ansible.playbook = "ansible/provision.yml"
    ansible.inventory_path = "ansible/development"
    ansible.limit = "vagrant"
  end
end
