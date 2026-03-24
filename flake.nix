{
  description = "lojix-macros — proc macros for deriving typed Rust from samskara schema";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    criome-cozo-src = { url = "github:LiGoldragon/criome-cozo"; flake = false; };
    samskara-core-src = { url = "github:LiGoldragon/samskara-core"; flake = false; };
    samskara-codegen-src = { url = "github:LiGoldragon/samskara-codegen"; flake = false; };
    samskara-src = { url = "github:LiGoldragon/samskara"; flake = false; };
    noesis-schema-src = { url = "github:LiGoldragon/noesis-schema"; flake = false; };
  };

  outputs = { self, nixpkgs, flake-utils, crane, fenix,
              criome-cozo-src, samskara-core-src, samskara-codegen-src,
              samskara-src, noesis-schema-src, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
        rustToolchain = fenix.packages.${system}.latest.toolchain;
        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        cozoFilter = path: _type: builtins.match ".*\\.cozo$" path != null;
        capnpFilter = path: _type: builtins.match ".*\\.capnp$" path != null;
        sourceFilter = path: type:
          (cozoFilter path type) || (capnpFilter path type) || (craneLib.filterCargoSources path type);
        src = pkgs.lib.cleanSourceWith {
          src = ./.;
          filter = sourceFilter;
        };

        commonArgs = {
          inherit src;
          pname = "lojix-macros";
          nativeBuildInputs = [ pkgs.capnproto ];
          postUnpack = ''
            mkdir -p $sourceRoot/flake-crates
            cp -rL ${criome-cozo-src} $sourceRoot/flake-crates/criome-cozo
            cp -rL ${samskara-core-src} $sourceRoot/flake-crates/samskara-core
            cp -rL ${samskara-codegen-src} $sourceRoot/flake-crates/samskara-codegen
            cp -rL ${samskara-src} $sourceRoot/flake-crates/samskara
            cp -rL ${noesis-schema-src} $sourceRoot/flake-crates/noesis-schema
          '';
        };
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;
      in
      {
        packages.default = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;
        });

        checks = {
          build = craneLib.buildPackage (commonArgs // {
            inherit cargoArtifacts;
          });
          tests = craneLib.cargoTest (commonArgs // {
            inherit cargoArtifacts;
          });
        };

        devShells.default = craneLib.devShell {
          packages = with pkgs; [ rust-analyzer sqlite jujutsu capnproto ];
        };
      }
    );
}
