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
            self.game.tick();
            self.unprocessed -= 1.0;
        }

        // JAVA: Thread.sleep(2) — makes a small pause
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

        // Java: SCALE = min(w/WIDTH, h/HEIGHT); image centered; rest black.
        let scale = (win_w as f32 / WIDTH as f32).min(win_h as f32 / HEIGHT as f32);
        let ww = (WIDTH as f32 * scale) as i32;
        let hh = (HEIGHT as f32 * scale) as i32;
        let xo = (win_w - ww) / 2;
        let yo = (win_h - hh) / 2;

        let pixels = &self.renderer.screen.pixels;
        buffer.fill(0);
        for dy in 0..hh {
            let sy = ((dy as f32 / scale) as i32).clamp(0, HEIGHT - 1);
            let dest_row = ((dy + yo) * win_w) as usize;
            let src_row = (sy * WIDTH) as usize;
            for dx in 0..ww {
                let sx = ((dx as f32 / scale) as i32).clamp(0, WIDTH - 1);
                buffer[dest_row + (dx + xo) as usize] =
                    (pixels[src_row + sx as usize] as u32) & 0x00FF_FFFF;
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
