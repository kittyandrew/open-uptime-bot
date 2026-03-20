# Source: https://blog.thalheim.io/2023/01/08/how-to-use-nixos-testing-framework-with-flakes/
# The first argument to this function is the test module itself
test:
# These arguments are provided by `flake.nix` on import, see checkArgs
{
  pkgs,
  system,
  oubot,
  oubot-cli ? null,
  docker-image ? null,
  test-script,
  test-script-type ? "python",
}: let
  tester-script-py = pkgs.stdenv.mkDerivation {
    name = "tester-script-py";
    buildInputs = [
      (pkgs.python3.withPackages
        (pythonPackages: with pythonPackages; [websockets requests]))
    ];
    unpackPhase = "true";
    installPhase = ''
      mkdir -p $out/bin $out/bin/lib
      cp ${./testbase.py} $out/bin/lib/testbase.py
      cp ${test-script} $out/bin/tester-script-py
      chmod +x $out/bin/tester-script-py
    '';
  };
  tester-script-sh = pkgs.stdenv.mkDerivation {
    name = "tester-script-sh";
    buildInputs = [pkgs.bash];
    unpackPhase = "true";
    installPhase = ''
      mkdir -p $out/bin
      cp ${test-script} $out/bin/tester-script-sh
      chmod +x $out/bin/tester-script-sh
    '';
  };
  tester-script = if test-script-type == "bash" then tester-script-sh else tester-script-py;
in
  pkgs.testers.runNixOSTest {
    node.specialArgs = {
      inherit oubot;
      inherit oubot-cli;
      inherit tester-script;
      inherit docker-image;
    };
    # This makes `self` available in the NixOS configuration of our virtual machines.
    # This is useful for referencing modules or packages from your own flake
    # as well as importing from other flakes.
    imports = [test];
  }
