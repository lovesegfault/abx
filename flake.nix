{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    fenix = {
      url = "github:figsoda/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, fenix, flake-utils, nixpkgs }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        fenixPkgs = fenix.packages.${system};
        rustFull = fenixPkgs.complete.withComponents [
          "cargo"
          "clippy-preview"
          "rust-src"
          "rust-std"
          "rustc"
          "rustfmt-preview"
        ];

        buildRustPackage = (pkgs.makeRustPlatform {
          cargo = rustFull;
          rustc = rustFull;
        }).buildRustPackage;
      in
      {
        defaultPackage = self.packages.${system}.abx;

        packages.abx = buildRustPackage {
          pname = "abx";
          version = "0.1.0";

          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;

          nativeBuildInputs = with pkgs; [
            pkg-config
          ];

          buildInputs = with pkgs; [
            gst_all_1.gstreamer
            gst_all_1.gst-plugins-base
            gst_all_1.gst-plugins-bad
            gst_all_1.gst-plugins-good
            glib
          ];
        };

        devShell = pkgs.mkShell {
          name = "abx";
          nativeBuildInputs = with pkgs; (self.packages.${system}.abx.nativeBuildInputs) ++ [
            cargo-edit
            cargo-udeps
            fenixPkgs.rust-analyzer
            nixpkgs-fmt
          ];
          buildInputs = with pkgs; (self.packages.${system}.abx.buildInputs) ++ [ ];
        };
      });
}
