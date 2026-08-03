//! The platform shell: window, events, timing, blit, audio device.
//!
//! This replaces Java's AWT pieces (`Initializer.createAndDisplayFrame`, the `Canvas` +
//! `BufferStrategy` in `Renderer`, and `InputHandler implements KeyListener`). Nothing in
//! here contains game logic; the game core is fully headless.

mod demo;
mod keys;

use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::PhysicalKey;
use winit::window::{Window, WindowId};

use crate::core::game::{self, Game};
use crate::core::renderer::{HEIGHT, Renderer, WIDTH};
use crate::core::updater;

struct App {
    game: Game,
    renderer: Renderer,
    window: Option<Rc<Window>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,

    demo: Option<demo::Demo>,

    // Java `Initializer.run()` timing state
    last_time: Instant,
    last_render: Instant,
    unprocessed: f64,
    frames: i32,
    ticks: i32,
    last_timer1: Instant,
}

/// Integer presentation scale and capped logical framebuffer dimensions.
pub fn logical_size_for_window(win_w: i32, win_h: i32) -> (i32, i32, i32) {
    let scale = (win_w / WIDTH).min(win_h / HEIGHT).clamp(1, 6);
    let w = (win_w / scale).clamp(WIDTH, 640);
    let h = (win_h / scale).clamp(HEIGHT, 400);
    (scale, w, h)
}

/// Blit the logical framebuffer into the window buffer at `scale`, centered at
/// `(xo, yo)`.
///
/// The window can legally be SMALLER than the logical screen: the logical size
/// clamps at 288x192 (`logical_size_for_window`), so any window below that — a
/// hard drag inward, a tiling WM, a restore from minimize — leaves the centering
/// offsets negative. Every destination write is therefore clipped to the window
/// rect instead of trusting the offsets; the old code cast a negative index to
/// `usize` and panicked out of bounds (a hard crash on resize, found in QA).
#[allow(clippy::too_many_arguments)]
pub fn blit_scaled(
    buffer: &mut [u32],
    win_w: i32,
    win_h: i32,
    pixels: &[i32],
    src_w: i32,
    scale: i32,
    xo: i32,
    yo: i32,
) {
    if win_w <= 0 || win_h <= 0 || scale <= 0 || src_w <= 0 {
        return;
    }
    let src_h = pixels.len() as i32 / src_w;
    for dy in 0..src_h * scale {
        let y = dy + yo;
        if y < 0 || y >= win_h {
            continue; // row falls outside the window
        }
        let dest_row = (y * win_w) as usize;
        let src_row = ((dy / scale) * src_w) as usize;
        for dx in 0..src_w * scale {
            let x = dx + xo;
            if x < 0 || x >= win_w {
                continue; // column falls outside the window
            }
            let (Some(dst), Some(src)) = (
                buffer.get_mut(dest_row + x as usize),
                pixels.get(src_row + (dx / scale) as usize),
            ) else {
                continue;
            };
            *dst = (*src as u32) & 0x00FF_FFFF;
        }
    }
}

impl App {
    fn new(game: Game, renderer: Renderer) -> App {
        let now = Instant::now();
        App {
            game,
            renderer,
            window: None,
            surface: None,
            demo: demo::Demo::from_env(),
            last_time: now,
            last_render: now,
            unprocessed: 0.0,
            frames: 0,
            ticks: 0,
            last_timer1: now,
        }
    }

    /// The body of Java `Initializer.run()`'s while loop, executed every `about_to_wait`.
    fn loop_iteration(&mut self, event_loop: &ActiveEventLoop) {
        if !self.game.running {
            event_loop.exit();
            return;
        }

        let now = Instant::now();
        let mut ns_per_tick = 1e9 / updater::NORM_SPEED as f64;
        if !self.game.display.menu_active() {
            ns_per_tick /= self.game.gamespeed as f64;
        }
        self.unprocessed += now.duration_since(self.last_time).as_nanos() as f64 / ns_per_tick;
        self.last_time = now;
        while self.unprocessed >= 1.0 {
            self.ticks += 1;
            if let Some(demo) = &mut self.demo {
                demo.on_tick(&mut self.game);
            }
            // apply a scripted `size:WxH` to the real window
            let pending = self.demo.as_mut().and_then(|d| d.pending_resize.take());
            if let (Some((w, h)), Some(window)) = (pending, &self.window) {
                let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(w, h));
                let (_, lw, lh) = logical_size_for_window(w as i32, h as i32);
                self.renderer.resize(lw, lh);
                self.game.screen_size = (lw, lh);
            }
            self.game.tick();
            self.unprocessed -= 1.0;
        }

        // brief pause so the loop yields the CPU between tick batches
        std::thread::sleep(Duration::from_millis(2));

        if now.duration_since(self.last_render).as_secs_f64() > 1.0 / self.game.max_fps as f64 {
            self.frames += 1;
            self.last_render = Instant::now();
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }

        if self.last_timer1.elapsed() > Duration::from_secs(1) {
            self.last_timer1 += Duration::from_secs(1);
            self.game.fra = self.frames;
            self.game.tik = self.ticks;
            self.frames = 0;
            self.ticks = 0;
        }
    }

    /// Draw the frame and blit it, scaled (Java `Renderer.render()`'s BufferStrategy part).
    fn redraw(&mut self) {
        self.renderer.render(&mut self.game);
        if let Some(demo) = &mut self.demo {
            demo.on_frame(&self.renderer);
        }

        let (Some(window), Some(surface)) = (&self.window, &mut self.surface) else {
            return;
        };

        let size = window.inner_size();
        let (win_w, win_h) = (size.width as i32, size.height as i32);
        if win_w <= 0 || win_h <= 0 {
            return;
        }
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

        let (scale, _, _) = logical_size_for_window(win_w, win_h);
        let ww = self.renderer.screen.w * scale;
        let hh = self.renderer.screen.h * scale;
        let xo = (win_w - ww) / 2;
        let yo = (win_h - hh) / 2;

        let pixels = &self.renderer.screen.pixels;
        buffer.fill(0);
        blit_scaled(
            &mut buffer,
            win_w,
            win_h,
            pixels,
            self.renderer.screen.w,
            scale,
            xo,
            yo,
        );

        let _ = buffer.present();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let scale = 3.0; // Java initial SCALE
        let attrs = Window::default_attributes()
            .with_title(game::NAME)
            .with_inner_size(LogicalSize::new(
                WIDTH as f64 * scale,
                HEIGHT as f64 * scale,
            ))
            .with_min_inner_size(LogicalSize::new(1.0, 1.0));
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
        event_loop.set_control_flow(ControlFlow::Poll);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                println!("window closing");
                self.game.quit();
                event_loop.exit();
            }
            WindowEvent::Focused(focused) => {
                self.game.has_focus = focused;
            }
            WindowEvent::KeyboardInput { event, .. } => {
                // AWT sent repeated keyPressed events while a key was held; winit marks
                // them with `repeat`, and the Key state machine expects them.
                if let PhysicalKey::Code(code) = event.physical_key {
                    if let Some(name) = keys::java_key_name(code) {
                        self.game
                            .input
                            .key_toggled(name, event.state == ElementState::Pressed);
                    } else {
                        println!("INPUT: Could not find keyname for key {code:?}");
                    }
                }
                if event.state == ElementState::Pressed {
                    if let Some(text) = &event.text {
                        for ch in text.chars() {
                            self.game.input.key_typed(ch);
                        }
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                self.redraw();
            }
            WindowEvent::Resized(size) => {
                let (_, w, h) = logical_size_for_window(size.width as i32, size.height as i32);
                self.renderer.resize(w, h);
                self.game.screen_size = (w, h);
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.loop_iteration(event_loop);
    }
}

/// Create the window and run the main loop (Java `Initializer.createAndDisplayFrame` +
/// `Initializer.run`). Blocks until the game quits.
pub fn run(game: Game, renderer: Renderer) {
    let event_loop = EventLoop::new().expect("could not create event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new(game, renderer);
    event_loop.run_app(&mut app).expect("event loop error");
    if app.game.debug {
        println!("main game loop ended; terminating application...");
    }
}
