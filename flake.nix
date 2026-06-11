{
  description = "Lex — beginner-first, memory-safe compiled language";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        lex = pkgs.rustPlatform.buildRustPackage {
          pname = "lex";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;

          # lex invokes rustc at runtime to compile generated code.
          nativeBuildInputs = [ pkgs.makeWrapper ];
          buildInputs = [ pkgs.rustc ];

          doCheck = true;

          postInstall = ''
            wrapProgram $out/bin/lex \
              --prefix PATH : "${pkgs.lib.makeBinPath [ pkgs.rustc pkgs.stdenv.cc ]}"
          '';

          meta = with pkgs.lib; {
            description = "Compiler for the Lex programming language";
            homepage = "https://github.com/HalcyonOmega/lex-lang";
            mainProgram = "lex";
            platforms = platforms.unix;
          };
        };
      in
      {
        packages.default = lex;
        packages.lex = lex;

        apps.default = {
          type = "app";
          program = "${lex}/bin/lex";
        };

        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            cargo
            rustc
            gcc
            lex
          ];
          shellHook = ''
            echo "Lex dev shell — rustc on PATH for \`lex run\`"
          '';
        };

        formatter = pkgs.nixfmt-rfc-style;
      }
    );
}
