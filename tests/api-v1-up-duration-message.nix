(import ./lib/lib.nix) {
  name = "api-v1-up-duration-message";

  nodes = {
    primary = import ./lib/primary.nix;
  };

  testScript = let
    c = import ./lib/config.nix;
  in ''
    primary.wait_for_unit("open-uptime-bot")
    primary.wait_for_open_port(${c.oubot-port})
    primary.succeed("tester-script-py")
  '';
}
