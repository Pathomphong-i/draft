class Dft < Formula
  desc "Draft — The Multiverse Version Control System for AI Agent Swarms"
  homepage "https://pathomphong-i.github.io/draft"
  license "GPL-2.0-only"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/Pathomphong-i/draft/releases/download/v0.2.0/dft-aarch64-apple-darwin.tar.gz"
    else
      url "https://github.com/Pathomphong-i/draft/releases/download/v0.2.0/dft-x86_64-apple-darwin.tar.gz"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/Pathomphong-i/draft/releases/download/v0.2.0/dft-aarch64-unknown-linux-gnu.tar.gz"
    else
      url "https://github.com/Pathomphong-i/draft/releases/download/v0.2.0/dft-x86_64-unknown-linux-gnu.tar.gz"
    end
  end

  def install
    bin.install "dft"
    bin.install_symlink "dft" => "draft"
    bin.install_symlink "dft" => "drf"
  end

  test do
    assert_match(/draft 0\.1\.0/i, shell_output("#{bin}/dft --version"))
    assert_match(/draft 0\.1\.0/i, shell_output("#{bin}/draft --version"))
  end
end
