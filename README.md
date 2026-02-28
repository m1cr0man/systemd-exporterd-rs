# systemd-exporterd-rs - Prometheus exporter for systemd units

Systemd-exporterd exports metrics on all systemd units on the system.

- Supports systemd-nspawn aliases

# TODO

- Monitor user services
- Handle services starting and stopping

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
