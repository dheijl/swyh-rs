{
  config,
  pkgs,
  lib,
  ...
}: {
  options.services.swyh = with lib.types;
    lib.mkOption {
      description = ''
        The Stream What You Hear service
      '';
      default = {};
      type = attrsOf (submodule {
        options = {
          enable = lib.mkEnableOption "Stream What You Hear service";
          server_port = lib.mkOption {
            description = "The port to listen on";
            type = port;
            default = 5901;
          };
          auto_resume = lib.mkOption {
            description = "Atomatically resume playing";
            type = bool;
            default = false;
          };
          sound_source = lib.mkOption {
            description = "Index of input device (null for default)";
            type = nullOr int;
            default = null;
          };
          log_level = lib.mkOption {
            description = "The log level: 'info' or 'debug'";
            type =
              str
              // {
                check = x: builtins.elem x ["info" "Info" "INFO" "debug" "Debug" "DEBUG"];
              };
            default = "info";
          };
          ssdp = lib.mkEnableOption "SSDP discovery";
          ssdp_interval_mins = lib.mkOption {
            description = "Timeout between ssdp discovery";
            type = numbers.positive;
            default = 10.0;
          };
          auto_reconnect = lib.mkOption {
            description = "Whether to reconnect ssdp device";
            type = bool;
            default = true;
          };
          use_wave_format = lib.mkOption {
            description = "Wheter to use wave format";
            type = bool;
            default = true;
          };
          bits_per_sample = lib.mkOption {
            description = "Bits per sample";
            type = ints.positive // {check = x: builtins.elem x [16 24];};
            default = 16;
          };
          streaming_format = lib.mkOption {
            description = "Streaming format: lpcm, wav, RF64 of flac";
            type =
              str
              // {
                check = x:
                  builtins.elem x [
                    "Lpcm"
                    "Wav"
                    "Rf64"
                    "Flac"
                  ];
              };
            default = "Lpcm";
          };
          monitor_rms = lib.mkOption {
            description = "Whether to enable RMS monitoring";
            type = bool;
            default = false;
          };
          capture_timeout = lib.mkOption {
            description = "Capture timeout";
            type = ints.positive;
            default = 2000;
          };
          inject_silence = lib.mkOption {
            description = "Whether to inject silence";
            type = bool;
            default = false;
          };
          package = lib.mkOption {
            description = ''
              The package to use. This is better to use "swyh-rs-cli" package when
              you are using it only as service, cause it does not require graphics
              dependencies. You are beter to set this to "swyh-rs" if you planned to
              use GUI version too, to optimize building time.
            '';
            type = package;
            default = pkgs.swyh-rs-cli;
          };
        };
      });
    };

  config = let
    cfg = config.services.swyh;
    filter = name: cfg: cfg.enable;
    mkService = name: cfg:
      lib.nameValuePair "swyh-${name}" {
        wantedBy = ["multi-user.target"];
        after = ["systemd-networkd-wait-online.service" "network-online.target" ];
        wants = ["systemd-networkd-wait-online.service" "network-online.target" ];
        serviceConfig = {
          ExecStart = ''
            ${cfg.package}/bin/swyh-rs-cli        \
              -C ${mkConfig name cfg}             \
              ${lib.optionalString cfg.ssdp "-x"}
          '';
          User = "swyh";
          LogsDirectory = "swyh";
          LogsDirectoryMode = "750";
        };
      };
    hasAny = lib.foldlAttrs (acc: name: cfg: acc || cfg.enable) false cfg;
    mkConfig = name: cfg: let
      configuration =
        (builtins.removeAttrs cfg ["package" "enable" "sound_source" "ssdp"])
        // (
          lib.optionalAttrs (cfg.sound_source != null) {
            sound_source_index = cfg.sound_source;
          }
        )
        // {
          config_id = "_${name}";
          config_dir = "/var/log/swyh";
        };
      format = pkgs.formats.toml {};
    in
      format.generate "config_${name}.toml" {inherit configuration;};
    ports = lib.mapAttrsToList (name: cfg: cfg.server_port) cfg;
  in {
    systemd.services = lib.mapAttrs' mkService cfg;
    users.users.swyh = lib.mkIf hasAny {
      group = "audio";
      isSystemUser = true;
    };
    networking.firewall.allowedTCPPorts = ports;
  };
}
