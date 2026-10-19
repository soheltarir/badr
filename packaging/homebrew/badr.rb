# Prepared release definition; publication requires separate release authorization.
class Badr < Formula
  desc "Manage local Business Rule and Architecture Decision Records"
  homepage "https://github.com/soheltarir/badr"
  version "0.1.0"
  license "MIT"

  on_macos do
    on_arm do
      depends_on macos: :sonoma
      url "https://github.com/soheltarir/badr/releases/download/v0.1.0/badr-0.1.0-aarch64-apple-darwin.tar.gz"
      sha256 "f37517b98f16b200452ed186d12bb1f2b5a3bb6ede4625731bf86e22381c4e14"
    end
    on_intel do
      depends_on macos: :sequoia
      url "https://github.com/soheltarir/badr/releases/download/v0.1.0/badr-0.1.0-x86_64-apple-darwin.tar.gz"
      sha256 "f6b262d135d52057a612aae0465635570efdcd5f4113e4c75d0780f54021140a"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/soheltarir/badr/releases/download/v0.1.0/badr-0.1.0-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "1a817f6cd2757543ecd0747603eee489c7adebc54c86ada75495e9321c8b6bc4"
    end
    on_intel do
      url "https://github.com/soheltarir/badr/releases/download/v0.1.0/badr-0.1.0-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "e1acc70d03d467339b2c1d5748aa969c8ea927d181861c660a964ae4ae053c97"
    end
  end

  def install
    bin.install "badr"
    pkgshare.install "docs", "BUILD-INFO.txt", "THIRD_PARTY_NOTICES.md"
  end

  def caveats
    <<~EOS
      Managed operations require trusted Git on PATH and a local Git worktree.
      Linux archives require glibc 2.35 or newer; see the release runtime matrix.
    EOS
  end

  test do
    assert_equal "badr #{version}", shell_output("#{bin}/badr --version").strip
    system "git", "init", "-b", "main", testpath/"repository"
    system "git", "-C", testpath/"repository", "-c", "user.name=Test",
           "-c", "user.email=test@example.invalid", "commit", "--allow-empty", "-m", "baseline"
    system bin/"badr", "--repo", testpath/"repository", "init", "--baseline", "main"
    system bin/"badr", "--repo", testpath/"repository", "validate"
  end
end
