# deployer

A tiny TCP server that listens for commands and triggers a self-deploy
on a server.

Currently depends server being provisioned with
[ansible](http://docs.ansible.com/).

# Table of Contents

- [Installation](#installation)
  - [Ubuntu](#ubuntu)
  - [Other OSes](#other-oses)
  - [Variables](#variables)
- [Client](#client)
  - [Message Format](#message-format)
  - [Message Example](#message-example)
  - [Errors](#errors)
- [Building](#building)
  - [Local](#local)
  - [Linux From Another OS](#linux-from-another-os)
- [Development](#development)
  - [Testing](#testing)
    - [Writing New Tests](#writing-new-tests)
  - [Releases](#releases)

# Installation

If you are deployer to an Ubuntu server you can use the playbook
provided in this repo to install `deployer` and the associated upstart
script to manage it.

## Ubuntu

Tested against Ubuntu 14.04 LTS

```bash
$ git clone https://github.com/brianloveswords/deployer.git
$ cd deployer
$ bin/install <host> <secret> <path_to_remote_playbook>
```

e.g., `bin/install 192.168.100.100 shhh-secret /srv/app/provision.yml`

## Other OSes

Make sure the remote machine has [ansible installed](http://docs.ansible.com/intro_installation.html#installing-the-control-machine). This is required so the machine can run playbooks against itself.

```bash
# this is on the server with the app you want to redeploy
# `deployer` should be somewhere on the path and the user running it
# should have whatever privileges necessary for the playbook
$ export DEPLOYER_SECRET="example-secret"
$ export DEPLOYER_PLAYBOOK="/path/to/playbook.yml"
$ export DEPLOYER_PORT=5189
$ nohup deployer > deployer.log &
```

## Variables

Variables can come from the environment or from ansible when
installing. Anything defined by ansible will overwrite whats in the
environment.

| Environment         | Ansible             | Description
|---------------------|---------------------|-------------
| `DEPLOYER_PORT`     | `deployer_port`     | Port to listen on. Defaults to **1469**
| `DEPLOYER_SECRET`   | `deployer_secret`   | Shared client/server secret.
| `DEPLOYER_PLAYBOOK` | `deployer_playbook` | Path to the playbook to run on the server


# Client

## Message Format

The client takes messages in JSON format with the following fields:

* `secret`: Should match `DEPLOYER_SECRET`
* `config`: Will be passed as [--extra-vars](http://docs.ansible.com/playbooks_variables.html#passing-variables-on-the-command-line) to the `ansible-playbook` command. Can contain any number of keys and values. **NOTE**: currently all values must be `String`s.

## Message Example

Assume `deployer` is running on `192.168.100.128` on port `1469`

```bash
$ echo '{"secret": "shhh", "config": {"test_var1": "Pico", "test_var2": "Loki"}}' |\
  nc 192.168.100.128 1469

okay, message received

PLAY [all] ********************************************************************

TASK: [store test_var1 in /tmp/test_var1] *************************************
changed: [127.0.0.1]

TASK: [store test_var2 in /tmp/test_var2] *************************************
changed: [127.0.0.1]

PLAY RECAP ********************************************************************
127.0.0.1                  : ok=2    changed=2    unreachable=0    failed=0

exit code: 0
okay, see ya later!
```

`deployer` pipes output of `ansible-command` back to the client
followed by the exit code.

## Errors
* If the secret is wrong, `deployer` sends "error, wrong secret" and
closes the connection.

* If the message couldn't be parsed because it's invalid JSON or has
missing/invalid fields, `deployer` will send "error, could not parse
message" and close the connection.

* If `ansible-playbook` couldn't be launched, `deployer` will send
"error, could not spawn ansible-playbook"

* If `ansible-playbook` exits with a non-zero exit code, its stderr will
  be sent followed by "exit code: <number>"

# Building

## Local

```bash
$ cargo build --release
```

The file will be output to `target/release/deployer`.

## Linux From Another OS

A `Vagrantfile` is provided and doing `vagrant up` will provision the
machine, build the binary and copy it back to the local machine to the
proper location for running the install playbook.

If you need to modify `src/main.rs` for any reason, be sure to rebuild
the linux binary by doing `make linux-build`.

# Development

## Testing

The test suite is an ansible playbook that builds `deployer`, installs
it on the build VM and sends a message to build a test playbook. You can
run it with:

```bash
$ make test
```

From a fresh start, expect the test to take ~5 minutes (event longer if
you don't have the VM image downloaded). Subsequent runs will be much
faster, ~15 seconds on my machine.

### Writing New Tests

Look at the following files to get a sense of how testing works:
* `deploy/ansible/test.yml`: Main test runner
* `test/test-playbook.yml`: Playbook that gets run by the deployer
* `test/test-vars.json`: JSON message that gets sent to the deployer

## Releases

While rust is still under active development it makes sense to check
releases into the repository. To create new binaries:

```
$ make release
```

**NOTE:** we currently assume the local machine is OS X, that will be
  fixed in the future

This runs the test suite (which builds the linux executable), then the
`local-build` task and copies the builds to `release/deployer.linux` and
`release/deployer.darwin`.
