{
  description = "systemd-exporterd-rs - Exports metrics on all systemd units on the system.";

  inputs = {
    nixpkgs.follows = "fenix/nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";

    crane = {
      url = "github:ipetkov/crane";
    };

    fenix = {
      url = "github:nix-community/fenix";
      # inputs.rust-analyzer-src.follows = "";
    };

    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };

  outputs = { self, nixpkgs, crane, fenix, flake-utils, advisory-db, ... }:
    {
      overlays = {
        systemd-exporterd-nixpkgs =
          let
            cargoConfig = (builtins.fromTOML (builtins.readFile "${self}/Cargo.toml"));
            pname = cargoConfig.package.name;
          in
          final: prev:
            let
              craneLib = crane.mkLib prev;
            in
            {
              ${pname} = final.rustPlatform.buildRustPackage {
                inherit pname;
                version = cargoConfig.package.version;
                src = craneLib.cleanCargoSource (craneLib.path self.outPath);
                buildFeatures = [ "cli" ];

                cargoLock.lockFile = "${self}/Cargo.lock";

                OPENSSL_NO_VENDOR = "1";
                PKG_CONFIG_PATH = "${prev.openssl.dev}/lib/pkgconfig";
                PKG_CONFIG = "${prev.pkg-config}/bin/pkg-config";

                meta = with prev.lib; {
                  description = "Systemd metric exporter";
                  homepage = "https://github.com/m1cr0man/systemd-exporterd";
                  license = licenses.mit;
                  maintainers = [ maintainers.m1cr0man ];
                };
              };
            };
      };

      nixosModules.systemd-exporterd = { config, pkgs, lib, ... }:
        let
          inherit (lib) types mkOption;
          cfg = config.systemd-exporterd;
          esa = lib.escapeShellArg;
          description = "Systemd metric exporter";
          user = "sd-exporterd";
        in
        {
          options.systemd-exporterd = {
            enable = lib.mkEnableOption description;

            listenerAddress = mkOption {
              type = types.str;
              default = "127.0.0.1:8080";
              description = "Address:port the exporter's HTTP server binds to.";
            };

            monitorUserManagers = mkOption {
              type = types.bool;
              default = false;
              description = ''
                Also enumerate active users via logind and monitor each
                per-user `systemd --user` instance.
              '';
            };

            includeFilters = mkOption {
              type = types.listOf types.str;
              default = [ ];
              example = [ "\\.service$" ];
              description = ''
                Regex patterns; only units matching at least one pattern are
                exported. Empty means include everything.
              '';
            };

            excludeFilters = mkOption {
              type = types.listOf types.str;
              default = [ ];
              example = [ "\\.device$" "\\.swap$" ];
              description = ''
                Regex patterns; units matching any pattern are dropped.
                Applied after includeFilters.
              '';
            };
          };

          config = lib.mkIf cfg.enable {
            users.users."${user}" = {
              group = user;
              home = "/var/empty";
              createHome = false;
              isSystemUser = true;
            };
            users.groups."${user}" = { };

            systemd.services.systemd-exporterd = {
              inherit description;
              after = [ "network-online.target" "local-fs.target" ];
              wantedBy = [ "multi-user.target" ];
              environment = {
                SDED_LISTENER_ADDRESS = cfg.listenerAddress;
                SDED_MONITOR_USER_MANAGERS = lib.boolToString cfg.monitorUserManagers;
              } // lib.optionalAttrs (cfg.includeFilters != [ ]) {
                SDED_INCLUDE_FILTERS = lib.concatStringsSep ":" cfg.includeFilters;
              } // lib.optionalAttrs (cfg.excludeFilters != [ ]) {
                SDED_EXCLUDE_FILTERS = lib.concatStringsSep ":" cfg.excludeFilters;
              };
              serviceConfig = {
                ExecStart = "${pkgs.systemd-exporterd}/bin/systemd-exporterd";
                RemainAfterExit = "no";
                User = user;
                Group = user;
                ProtectSystem = "full";
                PrivateTmp = "yes";
              };
            };
          };
        };

      nixosModule.systemd-exporterd-with-overlay = {
        imports = [
          self.nixosModules.systemd-exporterd
        ];
        nixpkgs.overlays = [ self.overlays.systemd-exporterd-nixpkgs ];
      };

    } //
    (flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
        };

        envVars = {
          OPENSSL_NO_VENDOR = "1";
          PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig";
          PKG_CONFIG = "${pkgs.pkg-config}/bin/pkg-config";
        };

        stdenv = p:
          if p.stdenv.isLinux then
            p.stdenvAdapters.useMoldLinker p.stdenv
          else
            p.stdenv;

        inherit (pkgs) lib;

        craneLib = (crane.mkLib pkgs).overrideScope (final: prev: {
          stdenvSelector = stdenv;
        });
        src = craneLib.cleanCargoSource ./.;

        mkToolchain = fenix.packages.${system}.combine;

        toolchain = fenix.packages.${system}.latest;

        buildToolchain = mkToolchain (with toolchain; [
          cargo
          rustc
        ]);

        craneLibBuild = craneLib.overrideToolchain buildToolchain;

        devToolchain = mkToolchain (with toolchain; [
          cargo
          clippy
          rust-src
          rustc
          llvm-tools
          rust-analyzer

          # Always use nightly rustfmt because most of its options are unstable
          fenix.packages.${system}.latest.rustfmt
        ]);

        craneLibDev = craneLib.overrideToolchain devToolchain;

        # Common arguments can be set here to avoid repeating them later
        commonArgs = {
          inherit src;
          strictDeps = true;

          buildInputs = [
            # Add additional build inputs here
          ] ++ lib.optionals pkgs.stdenv.isDarwin [
            # Additional darwin specific inputs can be set here
            pkgs.libiconv
          ];
        } // envVars;

        # Build *just* the cargo dependencies, so we can reuse
        # all of that work (e.g. via cachix) when running in CI
        cargoArtifacts = craneLibBuild.buildDepsOnly commonArgs;

        # Build the actual crate itself, reusing the dependency
        # artifacts from above.
        systemd-exporterd = craneLibBuild.buildPackage (commonArgs // {
          inherit cargoArtifacts;
          cargoExtraArgs = "-F cli";
        });
      in
      {
        checks = {
          # Build the crate as part of `nix flake check` for convenience
          inherit systemd-exporterd;

          # Run clippy (and deny all warnings) on the crate source,
          # again, resuing the dependency artifacts from above.
          #
          # Note that this is done as a separate derivation so that
          # we can block the CI if there are issues here, but not
          # prevent downstream consumers from building our crate by itself.
          systemd-exporterd-clippy = craneLibDev.cargoClippy (commonArgs // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets -- --deny warnings";
          });

          systemd-exporterd-doc = craneLibDev.cargoDoc (commonArgs // {
            inherit cargoArtifacts;
          });

          # Check formatting
          systemd-exporterd-fmt = craneLibDev.cargoFmt {
            inherit src;
          };

          # Audit dependencies
          # Broken for now
          # systemd-exporterd-audit = craneLib.cargoAudit {
          #   inherit src advisory-db;
          # };

          # Audit licenses
          systemd-exporterd-deny = craneLibDev.cargoDeny {
            inherit src;
          };

          # Run tests with cargo-nextest
          # Consider setting `doCheck = false` on `systemd-exporterd` if you do not want
          # the tests to run twice
          systemd-exporterd-nextest = craneLibDev.cargoNextest (commonArgs // {
            inherit cargoArtifacts;
            partitions = 1;
            partitionType = "count";
          });

          overlay = (import nixpkgs {
            inherit system;
            overlays = [ self.overlays.systemd-exporterd-nixpkgs ];
          }).systemd-exporterd;
        };

        packages = {
          inherit systemd-exporterd;
          default = systemd-exporterd;
          systemd-exporterd-lib = craneLibBuild.buildPackage (commonArgs // {
            inherit cargoArtifacts;
          });
          systemd-exporterd-llvm-coverage = craneLibDev.cargoLlvmCov (commonArgs // {
            inherit cargoArtifacts;
          });
          devTools = pkgs.linkFarm "vscode-dev-tools" {
            inherit (pkgs) nixpkgs-fmt gcc pkg-config;
            openssl = pkgs.openssl.dev;
            rust = devToolchain;
          };
        };

        apps.default = flake-utils.lib.mkApp {
          drv = systemd-exporterd;
        };

        devShells.default = craneLibDev.devShell
          ({
            # Inherit inputs from checks.
            checks = self.checks.${system};

            # Additional dev-shell environment variables can be set directly
            # MY_CUSTOM_DEVELOPMENT_VAR = "something else";
            RUST_SRC_PATH = "${devToolchain}/lib/rustlib/src/rust/library";
            MEGHAN = "bab";

            # Extra inputs can be added here; cargo and rustc are provided by default.
            packages = [
              # pkgs.ripgrep
            ];
          } // envVars);
      })
    );
}
