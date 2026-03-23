{
  pkgs,
  oubot,
  oubot-cli ? null,
  tester-script,
  ...
}: let
  c = import ./config.nix;
in {
  imports = [(import ./services.nix {inherit pkgs oubot;})];

  environment.systemPackages =
    [oubot tester-script]
    ++ (
      if oubot-cli != null
      then [oubot-cli]
      else []
    );
  environment.variables = {
    OUBOT_BASE_URL = "http://${c.host}:${c.oubot-port}";
    NTFY_BASE_URL = "${c.host}:${c.ntfy-port}";
  };
}
