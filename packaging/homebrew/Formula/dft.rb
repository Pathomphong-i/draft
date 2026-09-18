class Dft < Formula
  desc "Draft — The Multiverse Version Control System for AI Agent Swarms"
  homepage "https://pathomphong-i.github.io/draft"
  license "GPL-2.0-only"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/Pathomphong-i/draft/releases/download/v0.1.0/dft-aarch64-apple-darwin.tar.gz"
      sha256 "f72e9a7b2fee4dd251d176feee93ef98981fe72b954a50a3e7f053b4b29365df"
    else
      url "https://github.com/Pathomphong-i/draft/releases/download/v0.1.0/dft-x86_64-apple-darwin.tar.gz"
      sha256 "c1713c95e94afd463dac4d098db8d7fcb39d4356d152f95766009682a977b769"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/Pathomphong-i/draft/releases/download/v0.1.0/dft-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "efddbfad669b27adc954190802063568109133adac04581673c91fa74df60ddc"
    else
      url "https://github.com/Pathomphong-i/draft/releases/download/v0.1.0/dft-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "608dc8f764cdb184810740eb0980025ffca5b9e0d1dda2b5db346a5cdd6cca4e"
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
