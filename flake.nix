{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    fenix = {
      url = "github:figsoda/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.naersk.follows = "naersk";
    };
    naersk = {
      url = "github:nmattia/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, fenix, flake-utils, naersk, nixpkgs }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        fenixPkgs = fenix.packages.${system};
        naerskBuild = (naersk.lib.${system}.override {
          inherit (fenixPkgs.minimal) cargo rustc;
        }).buildPackage;
      in
      {
        defaultPackage = naerskBuild {
          src = ./.;
          buildInputs = with pkgs; [ gst_all_1.gstreamer ];
        };

        devShell = self.defaultPackage.${system}.overrideAttrs (oldAttrs: {
          buildInputs = with pkgs; (oldAttrs.buildInputs or [ ]) ++ [
            (fenixPkgs.complete.withComponents [
              "cargo"
              "clippy-preview"
              "rust-src"
              "rust-std"
              "rustc"
              "rustfmt-preview"
            ])
            fenixPkgs.rust-analyzer
            cargo-edit
            nixpkgs-fmt
          ];
        });
      });
}
