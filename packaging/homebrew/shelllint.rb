# Homebrew formula template for shelllint.
#
# Placeholders to replace at release time (the release job does this):
#   @VERSION@                     e.g. 0.0.1
#   @SHA256_X86_64_DARWIN@        sha256 of shelllint-v@VERSION@-x86_64-apple-darwin.tar.gz
#   @SHA256_AARCH64_DARWIN@       sha256 of shelllint-v@VERSION@-aarch64-apple-darwin.tar.gz
#   @SHA256_X86_64_LINUX@         sha256 of shelllint-v@VERSION@-x86_64-unknown-linux-musl.tar.gz
#   @SHA256_AARCH64_LINUX@        sha256 of shelllint-v@VERSION@-aarch64-unknown-linux-musl.tar.gz
#
# Users install with: brew install DevanshChoubey/tap/shelllint
# (the formula lives in a tap; copy this file to Formula/shelllint.rb there).

class Shelllint < Formula
  desc "Rust port of ShellCheck: finds bugs in shell scripts"
  homepage "https://github.com/DevanshChoubey/shelllint"
  version "@VERSION@"
  license "GPL-3.0-or-later"

  base_url = "https://github.com/DevanshChoubey/shelllint/releases/download/v#{version}"

  on_macos do
    if Hardware::CPU.arm?
      url "#{base_url}/shelllint-v#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "@SHA256_AARCH64_DARWIN@"
    else
      url "#{base_url}/shelllint-v#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "@SHA256_X86_64_DARWIN@"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "#{base_url}/shelllint-v#{version}-aarch64-unknown-linux-musl.tar.gz"
      sha256 "@SHA256_AARCH64_LINUX@"
    else
      url "#{base_url}/shelllint-v#{version}-x86_64-unknown-linux-musl.tar.gz"
      sha256 "@SHA256_X86_64_LINUX@"
    end
  end

  def install
    bin.install "shelllint"
  end

  test do
    (testpath/"bad.sh").write "#!/bin/bash\necho $foo\n"
    output = shell_output("#{bin}/shelllint -f gcc #{testpath}/bad.sh", 1)
    assert_match "SC2154", output
    assert_match "SC2086", output
  end
end
