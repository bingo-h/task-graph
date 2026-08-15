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
        extraBuildInputs = with pkgs; [
          pkg-config
          gtk3
          dbus
          atk
          webkitgtk_4_1
        ];
        # crossSystems = [
        #     nixpkgs.lib.systems.examples.aarch64-multiplatform
        #     nixpkgs.lib.systems.examples.wasm32
        #   ];
      };
    };
}
