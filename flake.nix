{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    crane.url = "github:ipetkov/crane";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs @ {
    flake-parts,
    nixpkgs,
    crane,
    self,
    ...
  }:
    flake-parts.lib.mkFlake {inherit inputs;} {
      systems = ["x86_64-linux"];
      perSystem = {
        config,
        self',
        inputs',
        pkgs,
        system,
        ...
      }: let
        craneLib =
          (crane.mkLib pkgs).overrideToolchain
          inputs.fenix.packages.${system}.minimal.toolchain;
        oubotRaw = craneLib.buildPackage {
          src = ./.;
          nativeBuildInputs = [pkgs.pkg-config];
          buildInputs = [pkgs.openssl pkgs.postgresql.lib];
        };
        oubot = pkgs.writeShellScriptBin "oubot" ''
          #!${pkgs.runtimeShell}

          # Run postgresql migrations.
          cp -r ${./migrations} ./migrations # Load migrations from source.
          ${pkgs.diesel-cli}/bin/diesel migration run

          # Finally, starting the actual program.
          ${oubotRaw}/bin/open-uptime-bot "$@"
        '';
        oubotCliRaw = craneLib.buildPackage {
          src = ./cli;
          nativeBuildInputs = [pkgs.pkg-config];
          buildInputs = [pkgs.openssl];
        };
        oubotCli = pkgs.runCommand "oubot-cli-wrapped" {
          nativeBuildInputs = [pkgs.makeWrapper];
        } ''
          mkdir -p $out/bin
          makeWrapper ${oubotCliRaw}/bin/oubot-cli $out/bin/oubot-cli \
            --prefix LD_LIBRARY_PATH : ${pkgs.lib.makeLibraryPath [pkgs.openssl]}
        '';
        toNanosec = seconds: seconds * 1000000000; # Specify nanoseconds as per docker spec (LMAO).
      in {
        formatter = pkgs.alejandra;

        packages = {
          server = oubot;
          cli = oubotCli;
          docker = pkgs.dockerTools.buildImage {
            name = "open-uptime-bot";
            tag = "2026.3.19";
            fromImage = pkgs.dockerTools.pullImage {
              imageName = "alpine";
              imageDigest = "sha256:25109184c71bdad752c8312a8623239686a9a2071e8825f20acb8f2198c3f659";
              sha256 = "sha256-gTKr5yQqJHECyXSyLA9GRT4Qm+ptahnRwy53W8Easb4=";
              finalImageTag = "3.23.3";
            };
            copyToRoot = pkgs.buildEnv {
              name = "image-root"; # @TODO: What is this name actually for?
              paths = [oubot oubotCli pkgs.curl]; # curl for healthcheck.
            };
            # @NOTE: Rocket.toml is very small and could be written here, but good for now.
            runAsRoot = ''
              #!${pkgs.runtimeShell}
              cp ${./Rocket.toml} /Rocket.toml
            '';
            config = {
              Cmd = ["${oubot}/bin/oubot"];
              Env = ["LD_LIBRARY_PATH=${pkgs.lib.makeLibraryPath [pkgs.openssl]}"];
              Healthcheck = {
                Test = ["CMD" "curl" "-sf" "0.0.0.0:8080/api/v1/health"];
                Interval = toNanosec 60;
                Timeout = toNanosec 3;
              };
            };
          };
        };

        devShells.default = let
          pythonCustom = pkgs.python3.withPackages (ps:
            with ps; [
              # Setup utils for packages and builds.
              pip
              wheel
              packaging
              setuptools
              # @TODO: Replace this with nix shell stuff.
              virtualenv # Local pythonic dev-env management.
              # Static analysis and formatting packages.
              flake8
              mypy
              black
              isort
              # Actual dependencies
              websockets
              aiohttp
              numpy
            ]);
          libs = with pkgs; [
            # Common libraries for python.
            stdenv.cc.cc
            zlib
            glib
            # Common libraries for rust.
            openssl
          ];

          custom-pico-sdk = with pkgs;
            pico-sdk.overrideAttrs (oldAttrs: rec {
              pname = "pico-sdk";
              version = "2.2.0";
              src = fetchFromGitHub {
                fetchSubmodules = true;
                owner = "raspberrypi";
                repo = pname;
                rev = version;
                sha256 = "sha256-8ubZW6yQnUTYxQqYI6hi7s3kFVQhe5EaxVvHmo93vgk=";
              };
            });
        in
          with pkgs;
            mkShell {
              RUST_LOG = "info";
              nativeBuildInputs = [pkg-config cmake gcc-arm-embedded git];
              buildInputs = [
                # Core server rust dev stuff.
                inputs.fenix.packages.${system}.complete.toolchain
                clippy
                rustc
                # Dev dependencies
                openssl
                bore-cli
                diesel-cli
                # Runtime dependency
                postgresql.lib
                # Pico W dev stuff.
                pythonCustom
                custom-pico-sdk
                picotool
                minicom
                rshell
              ];
              shellHook = ''
                export PICO_SDK_PATH=${custom-pico-sdk}/lib/pico-sdk/
                export LD_LIBRARY_PATH=${pkgs.lib.makeLibraryPath libs}

                cd clients/pico-w
                python -m virtualenv -q .venv && source .venv/bin/activate
                # We want to install additional requirements into a virtual env [for now].
                if [[ -f requirements.txt ]]; then
                  # @NOTE: In happy-case it is much cleaner to suppress the output. Is it bad? IDK.
                  python -m pip install -qr requirements.txt
                fi
                cd ../..

                echo -e "\nWelcome to the shell :)\n"
              '';
            };

        checks = let
          checkArgs = test-script: {
            inherit pkgs;
            inherit system;
            inherit test-script;
            inherit oubot;
          };
          checkArgsWithCliBash = test-script: {
            inherit pkgs;
            inherit system;
            inherit test-script;
            inherit oubot;
            oubot-cli = oubotCli;
            test-script-type = "bash";
          };
        in {
          api-v1-up-test-success = import ./tests/api-v1-up-test-success.nix (checkArgs ./tests/api-v1-up-test-success.py);
          api-v1-up-duration-message = import ./tests/api-v1-up-duration-message.nix (checkArgs ./tests/api-v1-up-duration-message.py);
          cli-lifecycle = import ./tests/cli-lifecycle.nix (checkArgsWithCliBash ./tests/cli-lifecycle.sh);
          cli-settings = import ./tests/cli-settings.nix (checkArgsWithCliBash ./tests/cli-settings.sh);
          cli-admin = import ./tests/cli-admin.nix (checkArgsWithCliBash ./tests/cli-admin.sh);
        };
      };
    };
}
