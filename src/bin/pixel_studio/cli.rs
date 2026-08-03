//! The command-line grammar: what the flags mean and how they parse.
//!
//! Parsing is separated from acting so the whole grammar is visible in one place and
//! the argument forms can be checked without touching the filesystem. Anything
//! malformed exits through [`usage`] with status 2; nothing here reads or writes art.

use std::path::PathBuf;

use crate::color::{PREVIEW_PALS, Rgba};

/// One headless pixel edit, applied in argument order.
pub(crate) enum BatchOp {
    Set(i32, i32, Rgba),
    Blit(i32, i32, i32, i32, i32, i32),
    Nudge(i32, i32),
}

/// Everything the command line asked for.
pub(crate) struct Args {
    pub(crate) target: Option<PathBuf>,
    /// `--sheet`: treat the target as a monolithic atlas even if a directory exists.
    pub(crate) force_sheet: bool,
    pub(crate) cell: Option<(i32, i32)>,
    pub(crate) size: i32,
    pub(crate) pal: usize,
    pub(crate) file_rel: Option<String>,
    pub(crate) shot: Option<PathBuf>,
    pub(crate) batch: Vec<BatchOp>,
    pub(crate) canvas_mode: bool,
    pub(crate) snap: Option<(i32, i32)>,
    pub(crate) new_sprite: Option<(String, String)>,
    pub(crate) backdrop: usize,
    pub(crate) demo_new: bool,
}

pub(crate) fn usage() -> ! {
    eprintln!(
        "usage: pixel_studio [<dir> | <sheet.png>] [--sheet <png>] [--cell X Y] [--size 8|16] [--pal N] [--backdrop N] [--canvas]\n       \
         pixel_studio <png> [--set X Y (RRGGBB[AA]|t)]... [--blit SX SY W H DX DY]... [--nudge DX DY]\n       \
         pixel_studio <dir> --file <rel.png> [--set ...]...   # headless edits resolve via the tree walk\n       \
         pixel_studio <dir> --canvas [--set ...]... [--blit ...]...  # headless edits in stitched-canvas coords\n       \
         pixel_studio <dir> --new <rel-no-ext> WxH            # create a blank sprite PNG (e.g. --new items/moonfruit 8x8)\n       \
         pixel_studio <target> --snap CX CY                   # print the sprite selection at cell CX,CY and exit\n       \
         pixel_studio <target> --shot <out.png>               # render one UI frame headlessly and exit\n\n\
         default target: assets/sprites (directory) if it exists, else assets/golden_atlas.png"
    );
    std::process::exit(2);
}

fn arg_i32(args: &[String], i: usize) -> i32 {
    args.get(i)
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| usage())
}

/// A `--set` colour: `RRGGBB`, `RRGGBBAA`, or `t` for transparent.
pub(crate) fn parse_set_color(s: &str) -> Option<Rgba> {
    if s.eq_ignore_ascii_case("t") || s.eq_ignore_ascii_case("transparent") {
        return Some([0, 0, 0, 0]);
    }
    let s = s.trim_start_matches('#');
    match s.len() {
        6 => u32::from_str_radix(s, 16)
            .ok()
            .map(|v| [(v >> 16) as u8, (v >> 8) as u8, v as u8, 255]),
        8 => u32::from_str_radix(s, 16)
            .ok()
            .map(|v| [(v >> 24) as u8, (v >> 16) as u8, (v >> 8) as u8, v as u8]),
        _ => None,
    }
}

impl Args {
    pub(crate) fn parse(args: &[String]) -> Args {
        let mut a = Args {
            target: None,
            force_sheet: false,
            cell: None,
            size: 16,
            pal: 0,
            file_rel: None,
            shot: None,
            batch: Vec::new(),
            canvas_mode: false,
            snap: None,
            new_sprite: None,
            backdrop: 0,
            demo_new: false,
        };
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--canvas" => a.canvas_mode = true,
                "--snap" => {
                    a.snap = Some((arg_i32(args, i + 1), arg_i32(args, i + 2)));
                    i += 2;
                }
                "--new" => {
                    let rel = args.get(i + 1).cloned().unwrap_or_else(|| usage());
                    let size = args.get(i + 2).cloned().unwrap_or_else(|| usage());
                    a.new_sprite = Some((rel, size));
                    i += 2;
                }
                "--backdrop" => {
                    a.backdrop = arg_i32(args, i + 1).clamp(0, 4) as usize;
                    i += 1;
                }
                "--demo-new" => a.demo_new = true,
                "--sheet" => {
                    a.target = Some(PathBuf::from(args.get(i + 1).unwrap_or_else(|| usage())));
                    a.force_sheet = true;
                    i += 1;
                }
                "--cell" => {
                    a.cell = Some((arg_i32(args, i + 1), arg_i32(args, i + 2)));
                    i += 2;
                }
                "--size" => {
                    a.size = arg_i32(args, i + 1);
                    i += 1;
                }
                "--pal" => {
                    a.pal = arg_i32(args, i + 1).clamp(0, PREVIEW_PALS.len() as i32 - 1) as usize;
                    i += 1;
                }
                "--file" => {
                    a.file_rel = Some(args.get(i + 1).cloned().unwrap_or_else(|| usage()));
                    i += 1;
                }
                "--shot" => {
                    a.shot = Some(PathBuf::from(args.get(i + 1).unwrap_or_else(|| usage())));
                    i += 1;
                }
                "--set" => {
                    let (x, y) = (arg_i32(args, i + 1), arg_i32(args, i + 2));
                    let c = args
                        .get(i + 3)
                        .and_then(|s| parse_set_color(s))
                        .unwrap_or_else(|| usage());
                    a.batch.push(BatchOp::Set(x, y, c));
                    i += 3;
                }
                "--blit" => {
                    a.batch.push(BatchOp::Blit(
                        arg_i32(args, i + 1),
                        arg_i32(args, i + 2),
                        arg_i32(args, i + 3),
                        arg_i32(args, i + 4),
                        arg_i32(args, i + 5),
                        arg_i32(args, i + 6),
                    ));
                    i += 6;
                }
                "--nudge" => {
                    a.batch
                        .push(BatchOp::Nudge(arg_i32(args, i + 1), arg_i32(args, i + 2)));
                    i += 2;
                }
                s if !s.starts_with('-') && a.target.is_none() => a.target = Some(PathBuf::from(s)),
                _ => usage(),
            }
            i += 1;
        }
        if ![8, 16].contains(&a.size) {
            eprintln!("--size must be 8 or 16");
            std::process::exit(2);
        }
        a
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_string).collect()
    }

    /// `--set` accepts the three documented colour forms, with or without a `#`.
    #[test]
    fn set_colours_parse_the_documented_forms() {
        assert_eq!(parse_set_color("FF00AA"), Some([0xFF, 0x00, 0xAA, 255]));
        assert_eq!(parse_set_color("#FF00AA"), Some([0xFF, 0x00, 0xAA, 255]));
        assert_eq!(parse_set_color("ff00aa"), Some([0xFF, 0x00, 0xAA, 255]));
        assert_eq!(
            parse_set_color("11223344"),
            Some([0x11, 0x22, 0x33, 0x44]),
            "RRGGBBAA keeps its alpha"
        );
        for t in ["t", "T", "transparent", "TRANSPARENT"] {
            assert_eq!(parse_set_color(t), Some([0, 0, 0, 0]), "{t}");
        }
    }

    /// Anything that is not a colour is refused rather than silently painted.
    #[test]
    fn malformed_colours_are_refused() {
        for bad in ["", "FFF", "GGGGGG", "FF00A", "FF00AA0", "0x112233"] {
            assert_eq!(parse_set_color(bad), None, "{bad:?} should not parse");
        }
    }

    /// A bare path is the target; flags fill in the rest.
    #[test]
    fn a_bare_path_is_the_target() {
        let a = Args::parse(&argv("assets/sprites"));
        assert_eq!(a.target, Some(PathBuf::from("assets/sprites")));
        assert!(!a.force_sheet && !a.canvas_mode);
        assert_eq!(a.size, 16, "the default window is 16px");
        assert_eq!(a.pal, 0);
        assert!(a.batch.is_empty());
    }

    /// `--sheet` both names the target and forces sheet mode.
    #[test]
    fn sheet_flag_forces_sheet_mode() {
        let a = Args::parse(&argv(
            "--sheet assets/golden_atlas.png --cell 15 26 --size 8",
        ));
        assert_eq!(a.target, Some(PathBuf::from("assets/golden_atlas.png")));
        assert!(a.force_sheet);
        assert_eq!(a.cell, Some((15, 26)));
        assert_eq!(a.size, 8);
    }

    /// Batch ops keep their argument order — later edits stack on earlier ones.
    #[test]
    fn batch_ops_keep_argument_order() {
        let a = Args::parse(&argv(
            "x.png --set 1 2 FF0000 --blit 0 0 4 4 8 8 --nudge -1 3 --set 5 6 t",
        ));
        assert_eq!(a.batch.len(), 4);
        assert!(matches!(a.batch[0], BatchOp::Set(1, 2, [255, 0, 0, 255])));
        assert!(matches!(a.batch[1], BatchOp::Blit(0, 0, 4, 4, 8, 8)));
        assert!(matches!(a.batch[2], BatchOp::Nudge(-1, 3)));
        assert!(matches!(a.batch[3], BatchOp::Set(5, 6, [0, 0, 0, 0])));
        assert_eq!(a.target, Some(PathBuf::from("x.png")));
    }

    /// Out-of-range preview indices clamp instead of panicking on lookup.
    #[test]
    fn preview_indices_clamp_into_range() {
        let a = Args::parse(&argv("d --pal 999 --backdrop 999"));
        assert_eq!(a.pal, PREVIEW_PALS.len() - 1);
        assert_eq!(a.backdrop, 4);

        let a = Args::parse(&argv("d --pal -5 --backdrop -5"));
        assert_eq!(a.pal, 0);
        assert_eq!(a.backdrop, 0);
    }

    /// The screenshot / snap / new-sprite hooks parse their operands.
    #[test]
    fn headless_hooks_parse_their_operands() {
        let a = Args::parse(&argv("d --snap 16 11 --shot out.png --demo-new"));
        assert_eq!(a.snap, Some((16, 11)));
        assert_eq!(a.shot, Some(PathBuf::from("out.png")));
        assert!(a.demo_new);

        let a = Args::parse(&argv("d --new items/moonfruit 8x8 --canvas"));
        assert_eq!(a.new_sprite, Some(("items/moonfruit".into(), "8x8".into())));
        assert!(a.canvas_mode);
    }
}
