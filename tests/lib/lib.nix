# Source: https://blog.thalheim.io/2023/01/08/how-to-use-nixos-testing-framework-with-flakes/
# The first argument to this function is the test module itself
test:
# These arguments are provided by `flake.nix` on import, see checkArgs
{
  pkgs,
  system,
  oubot,
  test-script,
}: let
  tester-script = pkgs.stdenv.mkDerivation {
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
in
  pkgs.testers.runNixOSTest {
    node.specialArgs = {
      inherit oubot;
      inherit tester-script;
    };
    # This makes `self` available in the NixOS configuration of our virtual machines.
    # This is useful for referencing modules or packages from your own flake
    # as well as importing from other flakes.
    imports = [test];
  }
