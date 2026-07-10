# systemd-exporterd-rs - Prometheus exporter for systemd units

Systemd-exporterd exports metrics on all systemd units on the system.

- Traverses into user sessions/managers.

## Configuration

### NixOS flake quick start

systemd-exporterd was built to be used in NixOS. You can use the module exported from
this repo's flake in your own configuration quite easily. The below snippet
is a stripped down version of [a basic NixOS flake](https://gist.github.com/m1cr0man/8cae16037d6e779befa898bfefd36627),
showing the important pieces.

```nix
{
  inputs = {
    # Extend the inputs
    systemd-exporterd.url = "github:m1cr0man/systemd-exporterd-rs";
  };

  outputs = { systemd-exporterd, ... }@inputs {
    nixosConfigurations = {
      myhost = {
        modules = [
          # Add systemd-exporterd to the module list
          systemd-exporterd.nixosModules.systemd-exporterd-with-overlay
          # Now configure systemd-exporterd
          ({ config, ... }: {
            services.systemd-exporterd = {
              enable = true;
              backends = {
                headscale = {
                  enable = true;
                  # Domain must match or be a subdomain of some frontend
                  domain = "ts.example.com";
                  addUserSuffix = true;
                  baseUrl = "https://headscale.example.com";
                  keyFile = "/var/run/secrets/my_headscale_key";
                };
                # You can enable > 1 backend per instance.
              };
              frontends = {
                cloudflare = {
                  enable = true;
                  domain = "example.com";
                  instanceId = config.networking.hostName;
                  # Requires Zone.DNS (DNS:Edit) permission on the domain
                  keyFile = "/var/run/secrets/my_cloudflare_key";
                };
                # You can enable > 1 frontend per instance.
              };
            };
          })
        ];
      };
    };
  };
}
```

### Nix quick start

If you are just using Nix as a package manager, you can quickly compile and
launch systemd-exporterd using this command:

```bash
nix run github:m1cr0man/systemd-exporterd-rs -- --help
```

### Other distributions

systemd-exporterd is configured through environment variables and CLI args. Check out
[the example config](./config.example.env) for a list of available options.

To keep API keys secure, you can specify a path to any `_API_KEY` option by
prefixing it with an `@` symbol. systemd-exporterd will read this file at runtime.

Here's some example invocations:

```bash
# Compile with cargo
cargo build . -F cli
# Test your configuration
$ systemd-exporterd --backends headscale,machinectl,jsonfile --frontends cloudflare --test
# Dry run the changes
$ systemd-exporterd --backends headscale,machinectl,jsonfile --frontends cloudflare --dry-run
# Do DNS Sync!
$ systemd-exporterd --backends headscale,machinectl,jsonfile --frontends cloudflare
```

## Development

This project uses Nix to manage the development environment.
Run `nix develop` for a shell with the Rust toolchain ready to go.

When using the VSCode workspace, link the devTools package to .dev
so that formatting, completions etc work correctly:

```bash
nix build --out-link .dev .#devTools
```

## Monitoring user managers

Set `SDED_ENABLE_USER_MANAGERS=true` to also enumerate active users via logind
and monitor each per-user `systemd --user` instance. Metrics from user managers
are exported with a `scope="user@<uid>"` label; system-manager metrics carry
`scope="system"`.

The exporter connects to each user's session bus at `$XDG_RUNTIME_DIR/bus`.
By default, the session `dbus-daemon` only accepts EXTERNAL auth from the UID
that owns it, so root gets rejected with a broken pipe. There are two ways to
grant root access.

### Option A: seteuid fallback (automatic, no configuration)

When the exporter runs as root and the initial connect fails with a broken
pipe, it retries the connect while temporarily calling `seteuid(<target uid>)`
so the session `dbus-daemon` sees the target user's UID via `SO_PEERCRED`.
Root is restored immediately after the handshake. This requires no per-host
configuration and is the default behaviour.

### Option B: session-bus policy (persistent, no `seteuid`)

If you'd rather have `dbus-daemon` allow root directly, drop an XML policy
snippet at `/etc/dbus-1/session-local.conf`:

```xml
<busconfig>
  <policy context="mandatory">
    <allow user="root"/>
  </policy>
</busconfig>
```

Then reload each running session bus (`kill -HUP <dbus-daemon-pid>`) or have
users log out and back in. After this, root can `connect()` to any
`$XDG_RUNTIME_DIR/bus` without needing the `seteuid` fallback.

Remove the file to revoke the grant:

```sh
rm /etc/dbus-1/session-local.conf
```
