# vhs demo for shellcheck-rust. Render from the repo root with the release
# binary mounted into the vhs image (git must be present in it too):
#
#   docker run --rm --memory=4g \
#     -v "$PWD:/vhs" \
#     -v "$PWD/target/release/shellcheck-rust:/usr/local/bin/shellcheck-rust:ro" \
#     ghcr.io/charmbracelet/vhs demo/demo.tape
#
# The fixture is copied to a scratch dir first, because `git apply` below
# would otherwise rewrite the tracked demo/buggy.sh on every render.
Output demo/demo.gif

Set Shell "bash"
Set FontSize 16
Set Width 1100
Set Height 620
Set TypingSpeed 40ms
Set Theme "Dracula"

Hide
Type "export PS1='$ '"
Enter
Type "cd $(mktemp -d) && cp /vhs/demo/buggy.sh . && clear"
Enter
Show

# 1. A script with a bug.
Type "cat buggy.sh"
Enter
Sleep 2s

# 2. Lint it. --color=always forces colors when the output is not a terminal.
Type "shellcheck-rust --color=always buggy.sh"
Enter
Sleep 4s

# 3. Emit a patch and apply it with git.
Type "clear"
Enter
Type "shellcheck-rust -f diff buggy.sh | git apply && cat buggy.sh"
Enter
Sleep 4s
