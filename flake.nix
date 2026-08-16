{
  inputs = {
    nixpkgs = {
      url = "github:NixOS/nixpkgs/nixos-unstable";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nix-config = {
      url = "git+ssh://git@github.com/bingo-h/nixos-config";
      inputs.nixpkgs.follows = "nixpkgs";
    };
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

      packages.${system}.default = nix-config.lib.mkTauriPackage {
        inherit pkgs;
        pname = "task-web"; # 修改为项目包名称
        version = "1.2.1";
        src = pkgs.fetchFromGitHub {
          owner = "bingo-h";
          repo = "task-web";
          rev = "v1.2.1";
          hash = "sha256-k40Qx+brC3HINGDu3ez8EwC/qxePGDE7uA7d+KDjLbo=";
        };

        srcTauriDir = "src-tauri";
        frontendDir = "frontend";

        # 第一次先用 fakeHash 跑,拿到真实哈希后替换
        npmDepsHash = "sha256-VFkKfM0s50kjgkjCXtCuUEtQiAz8/tDpiTPSm0uNbIY=";
        # npmDepsHash = "sha256-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx=";
      };

    };
}
