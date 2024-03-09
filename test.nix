{
  nixosTest,
  module,
  system,
  linkFarm,
}: let
  oldFlake = builtins.getFlake "github:dheijl/swyh-rs/90277704156a3caacbcc375a892188bb0e3a7029";
  oldPackage = oldFlake.packages."${system}".swyh-rs-cli;
  swyh-rs-cli-old =
    linkFarm "swyh-rs-cli-old"
    {
      "bin/swyh-rs-cli-old" = "${oldPackage}/bin/swyh-rs-cli";
    };
  modules = [module];
in
  nixosTest {
    name = "swyh";
    /*
    We have three tests here:
    1. Machine with service with default audio input device
    2. Machine with service with selected audio input device
    3. Machine with manually run of swyh-rs-cli
    First two just checked that services selects correct audio inputs
    TODO(Shvedov): We should to add test for samples going through loopbacks
    The third runs the old version of swyh-rs-cli (got with oldFlake above)
    utility to create default configuration before changes. Then it run actual
    (from current pkgs) to check whether it able to run with old configuration
    (migrate).
    TODO(Shvedov): Test the behaviour of configuration
    TODO(Shvedov): Add GUI tests with
      [OCR](https://nixos.org/manual/nixos/stable/#test-opt-enableOCR)
    */
    nodes.default = {
      imports = modules;
      services.swyh.test = {
        enable = true;
      };
      boot.kernelModules = ["snd-aloop"];
    };
    nodes.audio-1 = {
      imports = modules;
      services.swyh.test = {
        enable = true;
        sound_source = 1;
        auto_resume = true;
      };
      boot.kernelModules = ["snd-aloop"];
    };
    nodes.migration = {pkgs, ...}: {
      imports = modules;
      boot.kernelModules = ["snd-aloop"];
      environment.systemPackages = [
        swyh-rs-cli-old
        pkgs.swyh-rs-cli
      ];
    };
    testScript = {nodes, ...}: ''
      def read_logs(machine, path):
        from pathlib import Path
        p = Path(path)
        shared = machine.out_dir
        machine.copy_from_vm(path)
        res = ""
        with open(shared / p.name) as f:
          res = f.read()
        return res
      def check_machine(machine, card: str):
        machine.wait_for_unit("swyh-test.service")
        if card not in read_logs(machine, "/var/log/swyh/log_test.txt"):
            raise Exception("Wrong card selected")
      # Test first machine with default device
      check_machine(default, "Selected audio source: default:CARD=Loopback[#0]")
      # Test second machine with selected device
      check_machine(audio_1, "Selected audio source: sysdefault:CARD=Loopback[#1]")
      ### Migration test
      migration.wait_for_unit("network-online.target")
      # Run old version and check whether it created configuration and run
      # server
      (_, stdout) = migration.execute("swyh-rs-cli-old", check_return=False, timeout=5)
      logs = read_logs(migration, "/root/.swyh-rs/log_cli.txt")
      if "Creating a new default config /root/.swyh-rs/config_cli.toml" not in str(stdout):
        raise Exception("Old version creating config logs failed")
      if "The streaming server is listening on" not in logs:
        raise Exception("Old version start logs failed")
      # Run old version and check whether it reads configuration and run
      # server
      (_, stdout) = migration.execute("swyh-rs-cli", check_return=False, timeout=5)
      logs = read_logs(migration, "/root/.swyh-rs/log_cli.txt")
      if "Creating a new default config" in str(stdout):
        raise Exception("New version wrong creating config logs")
      if "The streaming server is listening on" not in logs:
        raise Exception("New version start logs failed")
    '';
  }
