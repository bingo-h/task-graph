{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    nix-config.url = "git+ssh://git@github.com/bingo-h/nixos-config";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      nix-config,
      ...
    }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs { inherit system; };
    in
    {
      devShells.${system}.default = nix-config.lib.mkRustShell {
        inherit pkgs rust-overlay;
        extraBuildInputs = [ ];
        # crossSystems = [
        #     nixpkgs.lib.systems.examples.aarch64-multiplatform
        #     nixpkgs.lib.systems.examples.wasm32
        #   ];
      };
    };
}
