{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  buildInputs = with pkgs; [
    # Rust development environment
    rustc
    cargo
    rustfmt
    clippy

    # Required system dependencies
    openssl
    pkgconf

    # Additional development tools
    git
  ];

  # Environment variables for OpenSSL
  OPENSSL_DIR = "${pkgs.openssl.dev}";
  OPENSSL_LIB_DIR = "${pkgs.openssl.out}/lib";
  OPENSSL_INCLUDE_DIR = "${pkgs.openssl.dev}/include";

  # Set PKG_CONFIG_PATH for proper library detection
  PKG_CONFIG_PATH = "${pkgs.openssl.dev}/lib/pkgconfig:${pkgs.pkgconf}/lib/pkgconfig";

  shellHook = ''
    echo "cargo-dirty development environment"
    echo "Rust version: $(rustc --version)"
    echo "Cargo version: $(cargo --version)"
    echo "OpenSSL: ${pkgs.openssl.version}"
  '';
}