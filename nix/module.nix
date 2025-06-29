self:
{
  pkgs,
  lib,
  config,
  ...
}:
let
  conf = config.services.tweakpoint;
  mkOption = lib.mkOption;
  key_code = with lib.types; enum (import ./key_codes.nix) // {
    description = "Evdev key code name, e.g. \"BTN_LEFT\" or \"KEY_A\"";
  };
  axis_code = with lib.types; enum (import ./axis_codes.nix);
  bus_type = with lib.types; enum (import ./bus_types.nix);
  action =
    with lib.types;
    let
      simple = enum [
        "None"
        "ToggleScroll"
      ];
      button = mkOptionType {
        name = "button";
        description = "{ Button = key_code }";
        check = x: lib.length (lib.attrNames x) == 1 && x ? Button && key_code.check x.Button;
        merge = lib.options.mergeEqualOption;
      };
      lock = mkOptionType {
        name = "lock";
        description = "{ ToggleLock = [ key_code ] }";
        check = x: lib.length (lib.attrNames x) == 1 && x ? ToggleLock && (listOf key_code).check x.ToggleLock;
        merge = lib.options.mergeEqualOption;
      };
    in
    oneOf [
      simple
      button
      lock
    ];
  axisDef =
    with lib.types;
    submodule {
      options = {
        axis = mkOption {
          type = axis_code;
          description = "Relative axis name";
          example = "REL_X";
        };
        factor = mkOption {
          type = float;
          description = "Factor between original axis and new axis movement";
          default = 1.0;
          example = 0.1;
        };
      };
    };
  filterNull =
    x:
    if lib.typeOf x == "set" then
      lib.pipe x [
        (lib.attrsets.filterAttrs (_: v: v != null))
        (lib.attrsets.mapAttrs (_: filterNull))
      ]
    else
      x;
  config_toml =
    ((pkgs.formats.toml { }).generate "tweakpoint.toml" (filterNull conf.settings)).overrideAttrs
      (old: {
        # dirty-dirty hack to run checks
        buildCommand = null;
        buildPhase = old.buildPhase or old.buildCommand;
        phases = [
          "buildPhase"
          "checkPhase"
        ];
        doCheck = true;
        checkPhase = ''
          ${old.checkPhase or ""}
          ${lib.getExe' self.packages.${pkgs.system}.default "tweakpoint"} --config $out --dump-config
        '';
      });
in
{
  options.services.tweakpoint = with lib.types; {
    enable = lib.mkEnableOption "tweakpoint service";
    logLevel = mkOption {
      type = enum [
        "trace"
        "debug"
        "info"
        "warn"
        "error"
      ];
      description = "Log level";
      default = "error";
      example = "info";
    };
    extraPkgs = mkOption {
      type = listOf package;
      description = "Extra packages to include into the search path";
      default = [ ];
      example = [ pkgs.xorg.xinput ];
    };
    postScript = mkOption {
      type = nullOr lines;
      description = "Script to run after tweakpoint starts";
      example = ''
        #!/usr/bin/env bash
        while ! xinput list-props pointer:tweakpoint; do sleep 0.1s; done
        xinput set-prop pointer:tweakpoint 'libinput Accel Profile Enabled' 0 1 0
        xinput set-prop pointer:tweakpoint 'libinput Accel Speed' 0.55
      '';
      default = null;
    };
    settingsFile = mkOption {
      type = oneOf [
        path
        package
      ];
      description = "Path to the config file. Overrides settings if set.
        Note that it won't be checked for correctness";
      default = config_toml;
    };
    settings = {
      socket_path = mkOption {
        type = nullOr path;
        description = "Path to the control socket";
        example = "/tmp/tweakpoint.sock";
        default = null;
      };
      device = mkOption {
        type = path;
        description = "Path to the input event device file";
        example = "/dev/input/by-id/usb-Foo-Bar";
      };
      btn_map = mkOption {
        type = attrsOf key_code;
        description = "Map buttons to other buttons";
        example = {
          BTN_MIDDLE = "BTN_LEFT";
          BTN_SIDE = "BTN_TASK";
        };
        default = { };
      };
      name = mkOption {
        type = str;
        description = "Human-readable name of the virtual pointer device";
        default = "tweakpoint";
        example = "tweakpoint";
      };
      vendor_id = mkOption {
        type = ints.u16;
        description = "Reported vendor_id of the virtual pointer device";
        default = 1;
      };
      product_id = mkOption {
        type = ints.u16;
        description = "Reported product_id of the virtual pointer device";
        default = 1;
      };
      product_version = mkOption {
        type = ints.u16;
        description = "Reported product version of the virtual pointer device";
        default = 1;
      };
      bus = mkOption {
        type = bus_type;
        description = "Reported bus type of the virtual pointer device";
        default = "BUS_USB";
      };
      axis_map = {
        regular = mkOption {
          type = attrsOf axisDef;
          description = "Map axis to other axis, when scroll mode disabled";
          example = {
            REL_WHEEL.axis = "REL_RESERVED";
          };
          default = { };
        };
        scroll = mkOption {
          type = attrsOf axisDef;
          description = "Map axis to other axis, when scroll mode enabled";
          example = {
            REL_Y = {
              axis = "REL_WHEEL_HI_RES";
              factor = -10.0;
            };
            REL_X = {
              axis = "REL_HWHEEL_HI_RES";
              factor = 10.0;
            };
          };
          default = { };
        };
      };
      hi_res_enabled = mkOption {
        type = bool;
        description = "Enable high-resolution wheel events?";
        default = true;
      };
      meta = {
        key = mkOption {
          type = key_code;
          description = "Meta key";
          example = "BTN_MIDDLE";
        };
        click = mkOption {
          type = action;
          description = "Click action";
          default = "None";
        };
        hold = mkOption {
          type = action;
          description = "Hold action";
          default = "None";
        };
        move = mkOption {
          type = action;
          description = "Move action; action performed when pointer is moved while meta button is pressed";
          default = conf.settings.meta.hold;
        };
        chord = mkOption {
          type = attrsOf action;
          description = "Action when other button is pressed together with the meta button";
          example = {
            BTN_LEFT = "ToggleScroll";
          };
          default = { };
        };
        hold_time = mkOption {
          type = str;
          description = "Hold timeout, with suffix s/ms/&c";
          example = "500ms";
          default = "250ms";
        };
      };
    };
  };
  config = # lib.mkIf conf.enable
    {
      systemd.user.services.tweakpoint = {
        Install.WantedBy = [ "default.target" ];
        Service =
          {
            Environment = "PATH=${
              lib.makeSearchPath "bin" (
                [
                  pkgs.bash
                  pkgs.coreutils
                ]
                ++ conf.extraPkgs
              )
            }";
            ExecStart = "${
              lib.getExe' self.packages.${pkgs.system}.default "tweakpoint"
            } --config ${config_toml}";
            Restart = "always";
          }
          // lib.mkIf (conf.postScript != null) {
            ExecStartPost = conf.postScript;
          };
      };
    };
}
