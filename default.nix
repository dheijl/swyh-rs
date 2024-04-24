{
  rustPlatform,
  pkg-config,
  cmake,
  rust,
  alsa-lib,
  xorg,
  pango,
  glib,
  cairo,
  mkShellNoCC,
  rust-analyzer-unwrapped,
  rust-bin,
  lib,
  withGui ? true,
  withCli ? true,
}:
assert withGui || withCli; let
  buildInputs =
    [
      alsa-lib
    ]
    ++ (lib.lists.optionals withGui [
      xorg.libXft
      xorg.libXext
      xorg.libXinerama
      xorg.libXcursor
      xorg.libXfixes
      cairo
      pango
      glib
    ]);
  nativeBuildInputs = [
    pkg-config
    cmake
  ];
  rust = rust-bin.stable.latest.default;
  pname =
    if withGui && withCli
    then "swyh-rs"
    else if withGui
    then "swyh-rs-gui"
    else "swyh-rs-cli";
  swyh-rs = rustPlatform.buildRustPackage rec {
    version = "1.10.5";
    inherit nativeBuildInputs buildInputs pname;
    # Filter-out generated, version-control and nix-related files to prevent
    # cache invalidation while editing them
    src = lib.cleanSourceWith {
      filter = path: type:
        with builtins; let
          bn = baseNameOf path;
        in
          (!lib.hasSuffix ".nix" bn) && bn != "flake.lock";
      src = lib.cleanSource ./.;
    };
    cargoLock.lockFile = ./Cargo.lock;
    buildNoDefaultFeatures = true;
    buildPhase =
      ''
        runHook preBuild

        safePostBuildHooks=("''${postBuildHooks[@]}")
        postBuildHooks=()
        preBuildHooks=()
      ''
      + (lib.optionalString withGui ''
        cargoBuildFeatures="gui"
        cargoBuildHook
      '')
      + (lib.optionalString withCli ''
        cargoBuildFeatures="cli"
        cargoBuildHook
      '')
      + ''
        postBuildHooks=("''${safePostBuildHooks[@]}")
        runHook postBuild
      '';
    postInstall =
      if withGui
      then ''
        mv $out/bin/swyh-rs $out/bin/swyh-rs-gui
        ln -s swyh-rs-gui $out/bin/swyh-rs
      ''
      else ''
        ln -s swyh-rs-cli $out/bin/swyh-rs
      '';
    meta = with lib; {
      description = "Stream What You Hear written in rust, inspired by SWYH";
      homepage = "https://github.com/dheijl/swyh-rs";
      license = licenses.mit;
      maintainers = [
        {
          name = "Yury Shvedov";
          email = "mestofel13@gmail.com";
          github = "ein-shved";
          githubId = 3513222;
        }
      ];
    };
  };
  devShell = mkShellNoCC {
    buildInputs =
      [
        rust
        rust-analyzer-unwrapped
      ]
      ++ buildInputs
      ++ nativeBuildInputs;
    RUST_SRC_PATH = "${rust}/lib/rustlib/src/rust/library";
  };
in
  swyh-rs
  // {
    inherit devShell;
  }
