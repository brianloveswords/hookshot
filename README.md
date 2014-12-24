# deployer

Add self-deployment capability to a site.

# Installation

If you are deployer to an Ubuntu server you can use the playbook
provided in this repo to install `deployer` and the associated upstart
script to manage it.

## Ubuntu

```bash
$ git clone
$ cd /path/to/deployer
$ DEPLOYER_SECRET=super-secret-stuff \
  DEPLOYER_PLAYBOOK=/mnt/bocoup.com/live/deploy/ansible/provision.yml \
  ansible-playbook -i 192.168.100.100, deploy/ansible/deploy.yml
```

Make sure the remote server has [ansible installed](http://docs.ansible.com/intro_installation.html#installing-the-control-machine).

```bash
# this is on the server with the app you want to redeploy
# `deployer` should be somewhere on the path and the user running it
# should have whatever privileges necessary for the playbook
$ export DEPLOYER_SECRET="example-secret"
$ export DEPLOYER_PLAYBOOK="/path/to/playbook.yml"
$ export DEPLOYER_PORT=5189
$ nohup deployer > deployer.log &
```

# Building

## Native

```bash
$ cargo build --release
```

The file will be output to `target/release/deployer`.

You can also use the following command to build & run in one step:

```bash
$ cargo run --release
```

Make sure to read below about Variables.

## Linux from another host

We deploy to linux boxes so users of another OS will need to use
Vagrant. A `Vagrantfile` is provided – doing `vagrant up` will provision
the machine, build the binary and copy it back to the local machine to
the proper location for deployment.

If you need to modify `src/main.rs` for any reason, be sure to rebuild
the linux binary by doing `vagrant provision`.

# Testing

The test suite is an ansible playbook that builds `deployer`, installs
it on the build VM and sends a message to build a test playbook. You can
run it with:

```bash
$ make test
```

From a fresh start, expect the test to take ~5 minutes (event longer if
you don't have the VM image downloaded). Subsequent runs will be much
faster, ~15 seconds on my machine.

## Writing New Tests

Look at the following files to get a sense of how testing works:
* `deploy/ansible/test.yml`: Main test runner
* `test/test-playbook.yml`: Playbook that gets run by the deployer
* `test/test-vars.json`: JSON message that gets sent to the deployer

# Variables

Variables can come from the environment or ansible. Anything defined by
ansible will overwrite whats in the environment.

| Environment         | Ansible             | Description
|---------------------|---------------------|-------------
| `DEPLOYER_PORT`     | `deployer_port`     | Port to listen on. Defaults to **1469**
| `DEPLOYER_SECRET`   | `deployer_secret`   | Shared client/server secret.
| `DEPLOYER_PLAYBOOK` | `deployer_playbook` | Path to the playbook to run on the server

## Example with CLI Ansible Variables

```bash
$ cd /path/to/deployer
$ ansible-playbook -i 192.168.100.100, deploy/ansible/deploy.yml \
  -e "deployer_secret=super-secret-stuff deployer_playbook=/mnt/bocoup.com/live/deploy/ansible/provision.yml"
```

# Usage

TODO: fill this out
