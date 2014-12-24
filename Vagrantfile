# -*- mode: ruby -*-
# vi: set ft=ruby :

VAGRANTFILE_API_VERSION = "2"

playbook =
  if ENV["TEST"]
    "deploy/ansible/test.yml"
  else
    "deploy/ansible/build.yml"
  end

vm_ip = "192.168.100.128"

Vagrant.configure(VAGRANTFILE_API_VERSION) do |config|
  config.vm.box = "ubuntu/trusty64"
  config.vm.network "private_network", ip: vm_ip
  config.ssh.forward_agent = true
  config.vm.provision "ansible" do |ansible|
    ansible.playbook = playbook
    ansible.inventory_path = "deploy/ansible/inventory/development"
    ansible.limit = "vagrant"
    ansible.extra_vars = { vm_ip: vm_ip  }
  end
end
