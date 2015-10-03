# deployer

Run tasks based on webhook messages. Goes great with GitHub push
webhook events.

# Install

- [Download the release](https://github.com/brianloveswords/deployer/releases)
  for your system architecture.

- (Optional) Put it in /usr/local/bin

- (Optional) Download and install an upstart script for it.

That's it. See the [Running Deployer](#running-deployer) section.

# Building from Source

- Ensure you have `libssl-dev` installed.
  - If you're on OS X you probably have this.

  - On Ubuntu you can do `apt-get install libssl-dev`. You'll probably
    need to `sudo`.

- Clone this repository or download a
  [source tarball](https://github.com/brianloveswords/deployer/releases)

- `make release`. When it's done the binary will be located at
  `./target/release/deployer`

# Running deployer

## Server Configuration

There is some quick upfront configuration necessary to start deployer. See an
annotated configuration example below:

```toml

## Every configuration requires a `config` section

[config]
port = 5712                      # Port to run deployer server
secret = "your v secure pass"    # Key for message verification
checkout_root = "/tmp/checkouts" # Directory to store checkouts. Must be writeable.
log_root = "/var/logs/deployer"  # Directory to store logs. Must be writeable

## `env.*` sections are optional. They represent extra data that will be sent to
## repositories that might need extra that shouldn't be stored in the repository
## configuration or embedded in the make or ansible tasks.

## Sections should be keyed by [env.{{user}}.{{repo}}.{{branch}}].  Keys within
## those sections must be strings but are otherwise arbitrary: they will be set
## as environment variables (case preserved), and passed as `--extra-vars`
## additionally when the task is ansible.

[env.brian.cool-website.production]
hostname = "cool-site.biz"
username = "admin"
password = "do you like geodes?"

[env.brian.cool-website.staging]
hostname = "staging.cool-site.biz"
username = "staging-admin"
password = "a passphrase for the stating server"
```

Use the `--config` command line parameter or the `DEPLOYER_CONFIG` environment
variable to tell deployer where the configuration file is.

**NOTE**: `deployer` loads and caches the configuration on startup.  If the
  configuration needs to change, that currently requires a restart of
  `deployer`. There will be a way to signal a configuration reload to deployer
  in a future version.

## Repository Configuration

`deployer` relies on a `.deployer.conf` file in the root of a repository to know
that it has tasks to run and to figure out what they are based on owner,
repository and branch. Below you can find an example annotated `deployer.conf`:

```toml
## All paths below are relative to the project root. For example, if the project
## is checked out /var/cool-website.biz, `deployer` will look for the default
## ansible playbook at /var/cool-website.biz/ansible/deploy.yml

## Defaults to use when a branch configuration is missing fields.
## "method" is required.
[default]
method = "ansible"               # default task type. "makefile" or "ansible"
task = "deploy"                  # default make task to run. Optional.
playbook = "ansible/deploy.yml"  # default playbook to use for ansible. Optional
inventory = "ansible/inventory"  # default inventory to use for ansible. Optional

## Configuration for branches that have tasks associated with them. This doesn't
## need to be comprehensive of every branch in the repository. Any configuration
## here will override any corresponding value from `default`.
[branch.production]
playbook = "deploy/production.yml"
inventory = "deploy/inventory/production"

## When the staging branch is pushed ansible-playbook will be run with default
## playbook and the "ansible/inventory/staging" inventory, doing a path lookup
## starting from the root of the repository.
[branch.staging]
inventory = "ansible/inventory/staging"

## When the prototype branch `make self-deploy` will be run instead of
## `ansible-playbook`. Any extra variables will be stored in the environment
## before running `make`.
[branch.prototype]
method = "makefile"
task = "self-deploy"

```

Now, assuming the `deployer` service is running at `http://bjb:4200`, set up a
webhook for the GitHub repository:

![screenshot of webhook setup](https://cldup.com/XsgQM-bHpr.png)

Now whenever the `production`, `staging` and `prototype` branches are pushed the
associated make task or ansible playbook/inventory combo will be executed.


## Inspecting status of a task

If you need to figure out the status of a job you can find out the ID by going
to the "Recent Deliveries" section of the associated repository. If a job was
rejected for any reason, github will show a red warning symbol next to the
delivery. If there is a green check, that means the task was scheduled
successfully (202 Accepted). Click to see more, and use the response tab to find
the job status URL:

![screeshot of the webhook delivery](https://cldup.com/Qvb1Mp7XKq.png)


In forthcoming versions we might expose an index of jobs per queue to make this
discovery easier.

# Design

`deployer` is designed to be flexible, fast, secure and reasonably safe.

## Flexible

Repositories do not need to be known ahead of time if they do not require
secrets, most configuration is done within the repo that wants to be
deployed.

Deployment tasks also don't have to be related to deploying a website -- for
example `deployer` itself has `.deployer.conf` which says to kick off a release
build every time a commit is pushed to master.

## Fast

We do the shallowest checkout possible to get the branch necessary. We also
allow unrelated tasks to run in parallel, e.g. `owner.repo.production` and
`owner.repo.staging` will be two different (shallow) checkouts that each
maintain their own queue of actions, so a production build will never block a
staging build.

## Secure

POST messages must be HMAC signed and they are verified before any action takes
place. In the future we may also add `owner` whitelisting.

## Reasonably Safe

Deployments of the same "fully qualified branch" -- that is
`{{owner}}.{{repo}}.{{branch}}` -- run in serial, and the local clone is hard
reset to the tip of the branch before any task runs. We are also considering
dockerizing (or otherwise containing) these checkout so any task runs within
them would be truly isolated.

# License

```text
The MIT License (MIT)

Copyright (c) <2015> <Brian J Brennan>

Permission is hereby granted, free of charge, to any person obtaining a copy of
this software and associated documentation files (the "Software"), to deal in
the Software without restriction, including without limitation the rights to
use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of
the Software, and to permit persons to whom the Software is furnished to do so,
subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS
FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR
COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER
IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN
CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
```
