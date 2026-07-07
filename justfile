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

# World-inspection map window: biome/tile view of a seed without playing. Tab toggles
# modes, arrows pan, +/- zoom, S screenshots (docs/DEV_GUIDE.md#world-inspection-worldview).
worldview seed="":
    cargo run --bin worldview -- {{seed}}

# Pixel-art studio: browse and edit the game's sprite PNGs in place. No target =
# assets/sprites (dir) if it exists, else assets/sprites.png. Pass a dir or a PNG
# (docs/DEV_GUIDE.md#pixel-art-studio-pixel_studio).
studio target="":
    cargo run --bin pixel_studio -- {{target}}

# Scripted run: boot to the title screen and screenshot it (target/verify/title.png).
demo-title:
    FDOOM_DEMO="wait:220;shot:{{verify_dir}}/title.png;quit" cargo run

# Screenshots land in target/verify/gen_menu.png and world.png.
# Scripted run: generate a world named PIT in a throwaway save dir and screenshot it.
demo-world:
    rm -rf "{{verify_dir}}/demo-save"
    FDOOM_DEMO="wait:220;key:ENTER;wait:5;type:P;type:I;type:T;wait:2;key:DOWN;wait:2;key:DOWN;wait:2;shot:{{verify_dir}}/gen_menu.png;key:ENTER;wait:600;shot:{{verify_dir}}/world.png;quit" \
        cargo run -- --savedir "{{verify_dir}}/demo-save"

# Keeps running after the world loads so you can play it; uses a throwaway save dir.
# Create and enter a fresh world with the given numeric seed (windowed).
seed n:
    #!/usr/bin/env sh
    set -e
    rm -rf "{{verify_dir}}/seed-save"
    script="wait:220;key:ENTER;wait:5"
    for c in $(printf 'SEED{{n}}' | fold -w1); do script="$script;type:$c"; done
    script="$script;wait:2;key:DOWN;wait:2"
    for c in $(printf '%s' "{{n}}" | fold -w1); do script="$script;type:$c"; done
    script="$script;wait:2;key:DOWN;wait:2;key:ENTER"
    FDOOM_DEMO="$script" cargo run -- --savedir "{{verify_dir}}/seed-save"

# 1 px per 4 tiles over a 4096-tile square, crosshair at the origin; for an
# interactive view use `just worldview <seed>`.
# Render the biome overview map for a seed (headless PNG in target/verify).
biome-map seed:
    FDOOM_SEED={{seed}} cargo test --test biome_frames biome_map_overview -- --nocapture
    @echo "map: {{verify_dir}}/biome_map_{{seed}}.png"

# Regenerate the sprite sheet from artgen and open it.
sheet:
    cargo run --bin artgen
    open assets/sprites.png 2>/dev/null || xdg-open assets/sprites.png

# Long randomized gameplay soak (release build: thousands of ticks across seeds).
soak:
    cargo test --release --test gameplay_soak -- --nocapture

# Run every visual test harness, then upscale everything in target/verify.
shots:
    cargo test --test biome_frames --test lighting --test hud_qol --test headless_render --test tides
    just upscale

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
