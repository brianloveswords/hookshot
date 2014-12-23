# deployer

Add self-deployment capability to a site.

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

# Variables

Variables can come from the environment or ansible. Anything defined by
ansible will overwrite whats in the environment.

| Environment         | Ansible             | Description
|---------------------|---------------------|-------------
| `DEPLOYER_SECRET`   | `deployer_secret`   | Shared client/server secret.
| `DEPLOYER_PLAYBOOK` | `deployer_playbook` | Path to the playbook to run on the server

## Example using Environment Variables

```bash
$ cd /path/to/deployer
$ DEPLOYER_SECRET=super-secret-stuff \
  DEPLOYER_PLAYBOOK=/mnt/bocoup.com/live/deploy/ansible/provision.yml \
  ansible-playbook -i 192.168.100.100, deploy/ansible/deploy.yml
```

## Example with CLI Ansible Variables

```bash
$ cd /path/to/deployer
$ ansible-playbook -i 192.168.100.100, deploy/ansible/deploy.yml \
  -e "deployer_secret=super-secret-stuff deployer_playbook=/mnt/bocoup.com/live/deploy/ansible/provision.yml"
```

# App Prerequisites

TODO: fill this out
