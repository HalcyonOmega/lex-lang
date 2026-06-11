# Dev environment: the lex compiler needs cargo/rustc to build, and the
# `lex` binary invokes rustc at runtime to verify/compile generated code.
{ pkgs ? import <nixpkgs> { } }:
pkgs.mkShell {
  packages = with pkgs; [ cargo rustc gcc ];
}
