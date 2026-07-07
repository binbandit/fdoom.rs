# Task runner for fdoom.rs — `just --list` shows everything.
# Docs: README.md, docs/DEV_GUIDE.md (FDOOM_DEMO syntax, cheat keys, troubleshooting).

# Directory scripted-run screenshots land in (absolute: FDOOM_DEMO shot paths are
# resolved relative to the game's cwd, so we always pass absolute paths).
verify_dir := justfile_directory() / "target" / "verify"

# Play the game.
run:
    cargo run

# Play with debug cheat keys enabled (see docs/DEV_GUIDE.md#debug-cheat-keys).
run-debug:
    cargo run -- --debug

# Unit + headless integration tests.
test:
    cargo test

# Everything CI would care about: format, lints as errors, tests.
check:
    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo test

# Scripted run: boot to the title screen and screenshot it (target/verify/title.png).
demo-title:
    FDOOM_DEMO="wait:220;shot:{{verify_dir}}/title.png;quit" cargo run

# Screenshots land in target/verify/gen_menu.png and world.png.
# Scripted run: generate a world named PIT in a throwaway save dir and screenshot it.
demo-world:
    rm -rf "{{verify_dir}}/demo-save"
    FDOOM_DEMO="wait:220;key:ENTER;wait:5;type:P;type:I;type:T;wait:2;key:DOWN;wait:2;key:DOWN;wait:2;shot:{{verify_dir}}/gen_menu.png;key:ENTER;wait:600;shot:{{verify_dir}}/world.png;quit" \
        cargo run -- --savedir "{{verify_dir}}/demo-save"

# Upscale target/verify PNGs 3x (288x192 is squint-sized) into *_3x.png copies.
upscale:
    #!/usr/bin/env sh
    set -e
    for f in "{{verify_dir}}"/*.png; do
        case "$f" in *_3x.png) continue ;; esac
        [ -e "$f" ] || { echo "no PNGs in {{verify_dir}} — run a demo-* recipe first"; exit 1; }
        sips --resampleWidth 864 "$f" --out "${f%.png}_3x.png" >/dev/null
        echo "wrote ${f%.png}_3x.png"
    done

# Path is the macOS game dir; Linux uses ~/.fdoom, Windows %APPDATA%/fdoom.
# DANGER: deletes ALL local saves, preferences, and unlocks.
clean-saves:
    rm -rf ~/fdoom
