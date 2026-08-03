//! The window shell: winit event loop, pointer hit-testing, and the blit to screen.
//!
//! The studio renders into a fixed 960x720 frame; this module scales that frame to
//! whatever size the window is, centered, nearest-neighbour — and inverts the same
//! transform to turn a cursor position back into frame coordinates. Everything the
//! pointer can land on is named by [`Hit`], so mouse handling reads as "what did they
//! click" rather than a pile of coordinate ranges.
//!
//! Redraws are on demand ([`ControlFlow::Wait`]); the only thing that animates itself
//! is the preview strip, which drives its own timer.

use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};
use winit::window::{Window, WindowId};

use crate::atlas::CELL;
use crate::layout::*;
use crate::studio::{Paint, Source, Studio, Tool};

pub(crate) enum Hit {
    SheetPane(i32, i32), // sheet px
    TreeRow(usize),      // entry index
    Canvas(i32, i32),    // block-relative px
    ShadeSwatch(usize),  // 0-3, 4 = transparent
    ColorSwatch(usize),
    RecentSwatch(usize),
    CustomSwatch,
    None,
}

pub(crate) struct App {
    pub(crate) st: Studio,
    pub(crate) window: Option<Rc<Window>>,
    pub(crate) surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    pub(crate) needs_render: bool,
    pub(crate) mods: ModifiersState,
    pub(crate) left_down: bool,
    pub(crate) mid_drag: Option<(i32, i32)>, // last cursor frame pos while middle-panning
    pub(crate) mid_acc: (i32, i32),
    pub(crate) cursor: Option<(i32, i32)>, // frame coords
    pub(crate) anim_next: Option<Instant>,
}

impl App {
    pub(crate) fn new(st: Studio) -> App {
        App {
            st,
            window: None,
            surface: None,
            needs_render: true,
            mods: ModifiersState::empty(),
            left_down: false,
            mid_drag: None,
            mid_acc: (0, 0),
            cursor: None,
            anim_next: None,
        }
    }

    pub(crate) fn refresh(&mut self) {
        self.needs_render = true;
        if let Some(w) = &self.window {
            w.set_title(&self.st.title());
            w.request_redraw();
        }
    }

    /* -------------------------------- hit testing -------------------------------- */

    fn hit(&self, fx: i32, fy: i32) -> Hit {
        if let Some(h) = self.hit_canvas(fx, fy) {
            return h;
        }
        if let Some(h) = self.hit_swatches(fx, fy) {
            return h;
        }
        if let Some(h) = self.hit_left_pane(fx, fy) {
            return h;
        }
        Hit::None
    }

    /// The edit canvas, in window-relative pixels (accounting for zoom and pan).
    fn hit_canvas(&self, fx: i32, fy: i32) -> Option<Hit> {
        let st = &self.st;
        let (_, _, bw, bh) = st.block_rect();
        let z = st.zoom();
        if (RX..RX + CANVAS_MAX).contains(&fx) && (CANVAS_Y..CANVAS_Y + CANVAS_MAX).contains(&fy) {
            let px = (fx - RX + st.pan.0) / z;
            let py = (fy - CANVAS_Y + st.pan.1) / z;
            if px < bw && py < bh {
                return Some(Hit::Canvas(px, py));
            }
        }
        None
    }

    /// The three palette banks, in the order they are stacked down the right pane.
    fn hit_swatches(&self, fx: i32, fy: i32) -> Option<Hit> {
        let st = &self.st;
        if (PAL_A_Y - 2..PAL_A_Y + 22).contains(&fy) && fx >= SWATCH_X {
            let i = (fx - SWATCH_X) / 26;
            if (0..=4).contains(&i) && (fx - SWATCH_X) % 26 < 22 {
                return Some(Hit::ShadeSwatch(i as usize));
            }
        }
        if (PAL_B_Y - 1..PAL_B_Y + 34).contains(&fy) && fx >= SWATCH_X {
            let cx = SWATCH_X + 12 * 17 + 10;
            if fx >= cx - 2 && fx < cx + 33 {
                return Some(Hit::CustomSwatch);
            }
            if fy < PAL_B_Y + 31 {
                let (coln, row) = ((fx - SWATCH_X) / 17, (fy - PAL_B_Y) / 17);
                let i = (row * 12 + coln) as usize;
                if coln < 12 && row < 2 && i < st.swatches.len() {
                    return Some(Hit::ColorSwatch(i));
                }
            }
        }
        if (RECENT_Y - 1..RECENT_Y + 16).contains(&fy) && fx >= SWATCH_X {
            let i = (fx - SWATCH_X) / 17;
            if i >= 0 && (i as usize) < st.recent.len() && (fx - SWATCH_X) % 17 < 15 {
                return Some(Hit::RecentSwatch(i as usize));
            }
        }
        None
    }

    /// The left pane: sheet pixels in sheet/canvas modes, a file row in dir mode.
    fn hit_left_pane(&self, fx: i32, fy: i32) -> Option<Hit> {
        let st = &self.st;
        if !(PANE_X..PANE_X + PANE_W).contains(&fx) || !(PANE_Y..PANE_Y + PANE_H).contains(&fy) {
            return None;
        }
        match &st.source {
            Source::Sheet | Source::Canvas { .. } => {
                let view = PANE_W / 2;
                let off_x = clamp_scroll(st.bx, st.view_w, st.img.w, view);
                let off_y = clamp_scroll(st.by, st.view_h, st.img.h, view);
                let (sx, sy) = (off_x + (fx - PANE_X) / 2, off_y + (fy - PANE_Y) / 2);
                if sx < st.img.w && sy < st.img.h {
                    return Some(Hit::SheetPane(sx, sy));
                }
            }
            Source::Tree {
                entries, scroll, ..
            } => {
                let row = *scroll + (fy - PANE_Y) / ROW_H;
                if row >= 0 && (row as usize) < entries.len() {
                    return Some(Hit::TreeRow(row as usize));
                }
            }
        }
        None
    }

    /* ---------------------------------- pointer ---------------------------------- */

    fn on_mouse_press(&mut self, button: MouseButton) {
        let Some((fx, fy)) = self.cursor else { return };
        if button == MouseButton::Middle {
            self.mid_drag = Some((fx, fy));
            self.mid_acc = (0, 0);
            return;
        }
        match (self.hit(fx, fy), button) {
            (Hit::Canvas(px, py), MouseButton::Left) => {
                if self.st.paste_armed {
                    self.st.paste_at(px, py);
                } else {
                    match self.st.tool {
                        Tool::Pencil => {
                            self.st.push_undo_block();
                            self.st.stamp(&[(px, py)]);
                            self.left_down = true;
                        }
                        _ => self.st.drag_anchor = Some((px, py)),
                    }
                }
            }
            (Hit::Canvas(px, py), MouseButton::Right) => self.st.eyedrop(px, py),
            (Hit::SheetPane(sx, sy), MouseButton::Left) => {
                self.st.set_origin(sx - sx % CELL, sy - sy % CELL);
            }
            (Hit::TreeRow(i), MouseButton::Left) => {
                self.st.open_entry(i, self.mods.shift_key());
            }
            (Hit::ShadeSwatch(4), MouseButton::Left) => self.st.cur = Paint::Erase,
            (Hit::ShadeSwatch(i), MouseButton::Left) => self.st.cur = Paint::Shade(i as u8),
            (Hit::ColorSwatch(i), MouseButton::Left) => {
                self.st.cur = Paint::Rgb(self.st.swatches[i]);
            }
            (Hit::RecentSwatch(i), MouseButton::Left) => {
                let c = self.st.recent[i];
                self.st.cur = if c[0] == c[1] && c[1] == c[2] {
                    Paint::Shade(c[0] / 64)
                } else {
                    Paint::Rgb(c)
                };
            }
            (Hit::CustomSwatch, MouseButton::Left) => self.st.cur = Paint::Custom,
            _ => return,
        }
        self.refresh();
    }

    fn on_mouse_release(&mut self, button: MouseButton) {
        match button {
            MouseButton::Middle => self.mid_drag = None,
            MouseButton::Left => {
                self.left_down = false;
                // releasing a shape drag commits it as one undoable stroke
                if let (Some((ax, ay)), Some((hx, hy))) = (self.st.drag_anchor, self.st.hover) {
                    let pts = self.st.shape_points(ax, ay, hx, hy);
                    if !pts.is_empty() {
                        self.st.push_undo_block();
                        self.st.stamp(&pts);
                    }
                }
                self.st.drag_anchor = None;
                self.refresh();
            }
            _ => {}
        }
    }

    fn on_cursor(&mut self, fx: i32, fy: i32) {
        self.cursor = Some((fx, fy));
        if self.mid_drag.is_some() {
            self.pan_with_middle_drag(fx, fy);
            return;
        }
        let hover = match self.hit(fx, fy) {
            Hit::Canvas(px, py) => Some((px, py)),
            _ => None,
        };
        self.st.sheet_hover = match self.hit(fx, fy) {
            Hit::SheetPane(sx, sy) => Some((sx, sy)),
            _ => None,
        };
        if self.left_down
            && self.st.tool == Tool::Pencil
            && let Some((px, py)) = hover
        {
            self.st.stamp(&[(px, py)]);
        }
        let changed = hover != self.st.hover;
        self.st.hover = hover;
        if changed || self.left_down || self.st.drag_anchor.is_some() || self.st.paste_armed {
            self.refresh();
        }
    }

    /// Middle-drag: pan the window origin across the image (free, per-pixel). The
    /// leftover sub-pixel motion accumulates so slow drags still track the cursor.
    fn pan_with_middle_drag(&mut self, fx: i32, fy: i32) {
        let Some((lx, ly)) = self.mid_drag else {
            return;
        };
        let z = self.st.zoom().max(1);
        self.mid_acc.0 += lx - fx;
        self.mid_acc.1 += ly - fy;
        self.mid_drag = Some((fx, fy));
        let (dx, dy) = (self.mid_acc.0 / z, self.mid_acc.1 / z);
        if dx != 0 || dy != 0 {
            self.mid_acc.0 -= dx * z;
            self.mid_acc.1 -= dy * z;
            self.st.set_origin(self.st.bx + dx, self.st.by + dy);
            self.refresh();
        }
    }

    /// Mouse wheel: zoom the canvas around the hovered pixel.
    fn on_wheel(&mut self, up: bool) {
        let Some((fx, fy)) = self.cursor else { return };
        if !(RX..RX + CANVAS_MAX).contains(&fx) || !(CANVAS_Y..CANVAS_Y + CANVAS_MAX).contains(&fy)
        {
            return;
        }
        let z = self.st.zoom();
        let nz = if up {
            (z * 5 / 4 + 1).min(48)
        } else {
            (z * 4 / 5).max(2)
        };
        if nz == z {
            return;
        }
        // keep the pixel under the cursor stationary
        let (ppx, ppy) = (
            (fx - RX + self.st.pan.0) / z,
            (fy - CANVAS_Y + self.st.pan.1) / z,
        );
        self.st.zoom_ovr = Some(nz);
        self.st.pan.0 = ppx * nz - (fx - RX);
        self.st.pan.1 = ppy * nz - (fy - CANVAS_Y);
        self.st.clamp_pan();
        self.refresh();
    }

    /// Esc closes the innermost thing that is open; with nothing open it quits,
    /// asking twice when there are unsaved edits. `true` means "exit now".
    fn on_escape(&mut self) -> bool {
        if self.st.help_on {
            self.st.help_on = false;
        } else if self.st.new_sprite.is_some() {
            self.st.new_sprite = None;
            self.st.status.clear();
        } else if self.st.find.is_some() {
            self.st.find = None;
            self.st.status.clear();
        } else if self.st.paste_armed {
            self.st.paste_armed = false;
            self.st.status.clear();
        } else if self.st.dirty && !self.st.esc_armed {
            self.st.esc_armed = true;
            self.st.status = "UNSAVED EDITS: ESC AGAIN TO QUIT, S TO SAVE, X TO REVERT".into();
        } else {
            return true;
        }
        self.refresh();
        false
    }

    /* ----------------------------------- blitting ----------------------------------- */

    /// Window coords -> internal frame coords (inverse of the `redraw` blit).
    fn to_frame(&self, px: f64, py: f64) -> Option<(i32, i32)> {
        let window = self.window.as_ref()?;
        let size = window.inner_size();
        let (win_w, win_h) = (size.width as i32, size.height as i32);
        let scale = (win_w as f32 / VIEW_W as f32).min(win_h as f32 / VIEW_H as f32);
        if scale <= 0.0 {
            return None;
        }
        let xo = (win_w - (VIEW_W as f32 * scale) as i32) / 2;
        let yo = (win_h - (VIEW_H as f32 * scale) as i32) / 2;
        let fx = ((px as f32 - xo as f32) / scale) as i32;
        let fy = ((py as f32 - yo as f32) / scale) as i32;
        ((0..VIEW_W).contains(&fx) && (0..VIEW_H).contains(&fy)).then_some((fx, fy))
    }

    /// Scaled nearest-neighbour blit, centered — same approach as worldview.
    fn redraw(&mut self) {
        if self.needs_render {
            self.st.render();
            self.needs_render = false;
        }
        let (Some(window), Some(surface)) = (&self.window, &mut self.surface) else {
            return;
        };
        let size = window.inner_size();
        let (win_w, win_h) = (size.width as i32, size.height as i32);
        let (Some(sw), Some(sh)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        else {
            return;
        };
        if surface.resize(sw, sh).is_err() {
            return;
        }
        let Ok(mut buffer) = surface.buffer_mut() else {
            return;
        };
        let scale = (win_w as f32 / VIEW_W as f32).min(win_h as f32 / VIEW_H as f32);
        let ww = (VIEW_W as f32 * scale) as i32;
        let hh = (VIEW_H as f32 * scale) as i32;
        let xo = (win_w - ww) / 2;
        let yo = (win_h - hh) / 2;
        buffer.fill(0);
        for dy in 0..hh {
            let sy = ((dy as f32 / scale) as i32).clamp(0, VIEW_H - 1);
            let dest_row = ((dy + yo) * win_w) as usize;
            let src_row = (sy * VIEW_W) as usize;
            for dx in 0..ww {
                let sx = ((dx as f32 / scale) as i32).clamp(0, VIEW_W - 1);
                buffer[dest_row + (dx + xo) as usize] = self.st.frame[src_row + sx as usize];
            }
        }
        let _ = buffer.present();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title(self.st.title())
            .with_inner_size(LogicalSize::new(VIEW_W as f64, VIEW_H as f64))
            .with_min_inner_size(LogicalSize::new(480.0, 360.0));
        let window = Rc::new(
            event_loop
                .create_window(attrs)
                .expect("could not create window"),
        );
        let context =
            softbuffer::Context::new(window.clone()).expect("could not create graphics context");
        let surface =
            softbuffer::Surface::new(&context, window.clone()).expect("could not create surface");
        self.window = Some(window);
        self.surface = Some(surface);
        event_loop.set_control_flow(ControlFlow::Wait);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(_) => {
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            WindowEvent::ModifiersChanged(m) => self.mods = m.state(),
            WindowEvent::CursorMoved { position, .. } => {
                if let Some((fx, fy)) = self.to_frame(position.x, position.y) {
                    self.on_cursor(fx, fy);
                } else {
                    self.cursor = None;
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let up = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y > 0.0,
                    MouseScrollDelta::PixelDelta(p) => p.y > 0.0,
                };
                self.on_wheel(up);
            }
            WindowEvent::MouseInput { state, button, .. } => match state {
                ElementState::Pressed => self.on_mouse_press(button),
                ElementState::Released => self.on_mouse_release(button),
            },
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed
                    && let PhysicalKey::Code(code) = event.physical_key
                {
                    if code == KeyCode::Escape {
                        if self.on_escape() {
                            event_loop.exit();
                        }
                        return;
                    }
                    self.on_key(code);
                }
            }
            WindowEvent::RedrawRequested => self.redraw(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.st.anim_on {
            let now = Instant::now();
            let next = *self.anim_next.get_or_insert(now);
            if now >= next {
                self.st.anim_advance();
                self.anim_next = Some(now + ANIM_FRAME);
                self.needs_render = true;
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            event_loop.set_control_flow(ControlFlow::WaitUntil(self.anim_next.unwrap()));
        } else {
            self.anim_next = None;
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }
}
