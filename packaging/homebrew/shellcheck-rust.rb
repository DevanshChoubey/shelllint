# Homebrew formula template for shellcheck-rust.
#
# Placeholders to replace at release time (the release job does this):
#   @VERSION@                     e.g. 0.0.1
#   @SHA256_X86_64_DARWIN@        sha256 of shellcheck-rust-v@VERSION@-x86_64-apple-darwin.tar.gz
#   @SHA256_AARCH64_DARWIN@       sha256 of shellcheck-rust-v@VERSION@-aarch64-apple-darwin.tar.gz
#   @SHA256_X86_64_LINUX@         sha256 of shellcheck-rust-v@VERSION@-x86_64-unknown-linux-musl.tar.gz
#   @SHA256_AARCH64_LINUX@        sha256 of shellcheck-rust-v@VERSION@-aarch64-unknown-linux-musl.tar.gz
#
# Users install with: brew install DevanshChoubey/tap/shellcheck-rust
# (the formula lives in a tap; copy this file to Formula/shellcheck-rust.rb there).

class ShellcheckRust < Formula
  desc "Rust port of ShellCheck: finds bugs in shell scripts"
  homepage "https://github.com/DevanshChoubey/shellcheck-rust"
  version "@VERSION@"
  license "GPL-3.0-or-later"

  base_url = "https://github.com/DevanshChoubey/shellcheck-rust/releases/download/v#{version}"

  on_macos do
    if Hardware::CPU.arm?
      url "#{base_url}/shellcheck-rust-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "@SHA256_AARCH64_DARWIN@"
    else
      url "#{base_url}/shellcheck-rust-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "@SHA256_X86_64_DARWIN@"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "#{base_url}/shellcheck-rust-v#{version}-aarch64-unknown-linux-musl.tar.gz"
      sha256 "@SHA256_AARCH64_LINUX@"
    else
      url "#{base_url}/shellcheck-rust-v#{version}-x86_64-unknown-linux-musl.tar.gz"
      sha256 "@SHA256_X86_64_LINUX@"
    end
  end

  def install
    bin.install "shellcheck-rust"
  end

  test do
    (testpath/"bad.sh").write "#!/bin/bash\necho $foo\n"
    output = shell_output("#{bin}/shellcheck-rust -f gcc #{testpath}/bad.sh", 1)
    assert_match "SC2154", output
    assert_match "SC2086", output
  end
end
