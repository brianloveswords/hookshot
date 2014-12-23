# -*- mode: ruby -*-
# vi: set ft=ruby :

# Vagrantfile API/syntax version. Don't touch unless you know what you're doing!
VAGRANTFILE_API_VERSION = "2"

Vagrant.configure(VAGRANTFILE_API_VERSION) do |config|
  config.vm.box = "ubuntu/trusty64"
  config.vm.network "forwarded_port", guest: 1469, host: 4169
  config.vm.network "private_network", ip: "192.168.100.128"
  config.ssh.forward_agent = true
  config.vm.provision "ansible" do |ansible|
    ansible.playbook = "deploy/ansible/build.yml"
    ansible.inventory_path = "deploy/ansible/inventory/development"
    ansible.limit = "vagrant"
  end
end
