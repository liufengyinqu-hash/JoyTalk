# Home-manager module for JoyTalk speech-to-text
#
# Provides a systemd user service for autostart.
# Usage: imports = [ joytalk.homeManagerModules.default ];
#        services.joytalk.enable = true;
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.services.joytalk;
in
{
  options.services.joytalk = {
    enable = lib.mkEnableOption "JoyTalk speech-to-text user service";

    package = lib.mkOption {
      type = lib.types.package;
      defaultText = lib.literalExpression "joytalk.packages.\${system}.joytalk";
      description = "The JoyTalk package to use.";
    };
  };

  config = lib.mkIf cfg.enable {
    systemd.user.services.joytalk = {
      Unit = {
        Description = "JoyTalk speech-to-text";
        After = [ "graphical-session.target" ];
        PartOf = [ "graphical-session.target" ];
      };
      Service = {
        ExecStart = "${cfg.package}/bin/joytalk";
        Restart = "on-failure";
        RestartSec = 5;
      };
      Install.WantedBy = [ "graphical-session.target" ];
    };
  };
}
