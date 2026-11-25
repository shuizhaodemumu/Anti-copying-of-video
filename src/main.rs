mod engine;
mod timing;

use engine::{ExperimentMedia, Strength};
use gtk4::{
    gdk, gio, glib, prelude::*, Application, ApplicationWindow, Box as Row, Button, Label,
    Orientation, Picture, ToggleButton,
};
use std::{cell::RefCell, rc::Rc};
use timing::{Cadence, Stability, Tick};

const APP_ID: &str = "io.github.shuizhaodemumu.Photo60Test";
const NORMAL_HINT: &str = "先拍原画，按空格开始。按 5 看翻拍暗字，按 6 比较减闪档；Esc 返回原画。";
fn active_hint(strength: Strength, scene: Scene) -> &'static str {
    match (strength, scene) {
        (Strength::Latent, Scene::Text) => "拍摄整页，对照九宫格每一格的暗字。按 6 切换减闪档。",
        (Strength::Latent, Scene::Video) => "拍摄画面上方正中的暗字。按 6 对照减闪档。",
        (Strength::Balanced, Scene::Text) => {
            "对照现场和录像，看九格中哪些能读出暗字。按 5 对照原档。"
        }
        (Strength::Balanced, Scene::Video) => {
            "观察上方暗字和画面闪动。按 5 对照原档，Esc 返回原画。"
        }
        (Strength::Standard, _) => "标准档与 0.1.0 相同。可直接切换增强档比较；Esc 返回原画。",
        (Strength::Strong, _) => {
            "增强档会改变笔画边缘的对比度。请对照小字和字幕；出现不适可按 Esc 返回原画。"
        }
    }
}
const TIMING_HINT: &str = "已显示原画。请将此屏设为固定 60 Hz，等刷新稳定后再试。";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Scene {
    Text,
    Video,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Original,
    A,
    B,
}

struct State {
    scene: Scene,
    experiment: bool,
    strength: Strength,
    next_is_b: bool,
    paired_frame: usize,
    paused: bool,
    position_us: i64,
    last_tick: Option<i64>,
    rendered: Option<(Scene, usize, Phase)>,
    cadence: Cadence,
    stability: Stability,
    monitor_hz: Option<f64>,
    last_status: i64,
    media_error: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            scene: Scene::Text,
            experiment: false,
            strength: Strength::Latent,
            next_is_b: false,
            paired_frame: 0,
            paused: false,
            position_us: 0,
            last_tick: None,
            rendered: None,
            cadence: Cadence::default(),
            stability: Stability::default(),
            monitor_hz: None,
            last_status: -1_000_000,
            media_error: false,
        }
    }
}

struct Viewer {
    window: glib::WeakRef<ApplicationWindow>,
    picture: Picture,
    text_button: Button,
    video_button: Button,
    standard_button: ToggleButton,
    strong_button: ToggleButton,
    latent_button: ToggleButton,
    balanced_button: ToggleButton,
    experiment_button: Button,
    pause_button: Button,
    restart_button: Button,
    rate: Label,
    mode: Label,
    play_time: Label,
    hint: Label,
    media: RefCell<ExperimentMedia>,
    state: RefCell<State>,
}

impl Viewer {
    fn controls(&self) {
        let s = self.state.borrow();
        let video = s.scene == Scene::Video;
        for (button, selected) in [(&self.text_button, !video), (&self.video_button, video)] {
            if selected {
                button.add_css_class("selected")
            } else {
                button.remove_css_class("selected")
            }
        }
        self.pause_button.set_visible(video);
        self.restart_button.set_visible(video);
        self.play_time.set_visible(video);
        self.pause_button.set_label(if s.paused {
            "继续播放"
        } else {
            "暂停视频"
        });
        self.mode.set_text(match (s.experiment, s.strength) {
            (false, _) => "原画",
            (true, Strength::Standard) => "标准实验",
            (true, Strength::Strong) => "增强实验",
            (true, Strength::Latent) => "暗字实验",
            (true, Strength::Balanced) => "减闪实验",
        });
        if s.experiment {
            self.experiment_button.set_label("返回原画");
            self.experiment_button.add_css_class("stop");
        } else {
            self.experiment_button.set_label("开始实验");
            self.experiment_button.remove_css_class("stop");
        }
    }

    fn original(&self, message: &str) {
        {
            let mut s = self.state.borrow_mut();
            s.experiment = false;
            s.next_is_b = false;
        }
        self.hint.set_text(message);
        self.controls();
        self.draw();
    }

    fn toggle_mode(&self) {
        let s = self.state.borrow();
        let active = s.experiment;
        let ready = !s.media_error && s.cadence.suitable(s.monitor_hz);
        drop(s);
        if active {
            self.original(NORMAL_HINT);
        } else if ready {
            let mut s = self.state.borrow_mut();
            s.experiment = true;
            s.stability = Stability::default();
            s.next_is_b = false;
            let hint = active_hint(s.strength, s.scene);
            drop(s);
            self.hint.set_text(hint);
            self.controls();
        } else {
            self.hint
                .set_text("实验尚未开启。请检查此屏为固定 60 Hz，等回调稳定约一秒后重试。");
        }
    }

    fn select_strength(&self, strength: Strength) {
        let mut state = self.state.borrow_mut();
        if state.strength == strength {
            return;
        }
        self.media.borrow_mut().set_strength(strength);
        state.strength = strength;
        state.rendered = None;
        state.next_is_b = false;
        // The next display callback starts a complete A/B pair, even when paused.
        if state.experiment {
            self.hint.set_text(active_hint(strength, state.scene));
        }
        drop(state);
        self.controls();
    }

    fn select(&self, scene: Scene) {
        let mut s = self.state.borrow_mut();
        if s.scene == scene {
            return;
        }
        s.scene = scene;
        s.position_us = 0;
        s.last_tick = None;
        s.paused = false;
        s.media_error = false;
        s.last_status = -1_000_000;
        drop(s);
        self.original(NORMAL_HINT);
    }

    fn pause(&self) {
        let mut s = self.state.borrow_mut();
        if s.scene != Scene::Video {
            return;
        }
        s.paused = !s.paused;
        s.last_tick = None;
        drop(s);
        self.controls();
    }

    fn restart(&self) {
        let mut s = self.state.borrow_mut();
        if s.scene == Scene::Video {
            s.position_us = 0;
            s.last_tick = None;
            s.next_is_b = false;
            s.last_status = -1_000_000;
        }
    }

    fn fullscreen(&self) {
        if let Some(win) = self.window.upgrade() {
            if win.is_fullscreen() {
                win.unfullscreen()
            } else {
                win.fullscreen()
            }
        }
    }

    // Only the display frame callback advances experimental A/B. B always uses A's source frame.
    fn draw(&self) {
        let mut s = self.state.borrow_mut();
        if s.media_error {
            return;
        }
        let mut media = self.media.borrow_mut();
        let mut index = match s.scene {
            Scene::Text => 0,
            Scene::Video => ((s.position_us * 30 / 1_000_000) as usize) % media.frame_count(),
        };
        let phase = if !s.experiment {
            Phase::Original
        } else if s.next_is_b {
            index = s.paired_frame;
            s.next_is_b = false;
            Phase::B
        } else {
            s.paired_frame = index;
            s.next_is_b = true;
            Phase::A
        };
        let key = (s.scene, index, phase);
        if s.rendered == Some(key) {
            return;
        }
        let result = if s.scene == Scene::Text {
            Ok(media.text())
        } else {
            media.video(index)
        };
        match result {
            Ok(frames) => {
                let texture = match phase {
                    Phase::Original => &frames.normal,
                    Phase::A => &frames.a,
                    Phase::B => &frames.b,
                };
                self.picture.set_paintable(Some(texture));
                s.rendered = Some(key);
            }
            Err(err) => {
                s.experiment = false;
                s.media_error = true;
                s.rendered = None;
                self.picture.set_paintable(gdk::Paintable::NONE);
                drop(s);
                self.hint.set_text(&format!("视频帧读取失败：{err}"));
                self.controls();
            }
        }
    }

    fn tick(&self, now: i64) {
        let mut s = self.state.borrow_mut();
        let tick = s.cadence.observe(now);
        if tick == Tick::Duplicate {
            return;
        }
        let elapsed = s.last_tick.replace(now).map(|then| now - then).unwrap_or(0);
        if s.scene == Scene::Video && !s.paused {
            // Discard time while the window was hidden, rather than skipping the whole clip.
            if (0..=250_000).contains(&elapsed) {
                let duration = self.media.borrow().frame_count() as i64 * 1_000_000 / 30;
                s.position_us = (s.position_us + elapsed) % duration;
            }
        }
        if now - s.last_status >= 350_000 {
            s.last_status = now;
            s.monitor_hz = self.window.upgrade().and_then(|w| {
                let surface = w.surface()?;
                let hz = gtk4::prelude::WidgetExt::display(&w)
                    .monitor_at_surface(&surface)?
                    .refresh_rate();
                (hz > 0).then_some(hz as f64 / 1_000.0)
            });
            let rate = s
                .cadence
                .hz()
                .map(|hz| format!("回调 {hz:.1} Hz"))
                .unwrap_or_else(|| "回调测量中…".to_owned());
            self.rate.set_text(&rate);
            if s.scene == Scene::Video {
                self.play_time.set_text(&format!(
                    "{:02}.{:01} s",
                    s.position_us / 1_000_000,
                    s.position_us % 1_000_000 / 100_000
                ));
            }
        }
        let monitor_hz = s.monitor_hz;
        let suitable = s.cadence.suitable(monitor_hz);
        let unstable = s.experiment && s.stability.stop(now, tick, suitable, monitor_hz);
        drop(s);
        if unstable {
            self.original(TIMING_HINT)
        } else {
            self.draw()
        }
    }

    fn bind(self: &Rc<Self>, button: &Button, action: impl Fn(&Self) + 'static) {
        let weak = Rc::downgrade(self);
        button.connect_clicked(move |_| {
            if let Some(view) = weak.upgrade() {
                action(&view);
            }
        });
    }
}

fn make_button(label: &str, tip: &str) -> Button {
    let button = Button::with_label(label);
    button.set_tooltip_text(Some(tip));
    button
}

fn spacer() -> Row {
    let spacer = Row::new(Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    spacer
}

fn build(app: &Application) {
    if let Some(existing) = app.active_window() {
        existing.present();
        return;
    }
    let css = gtk4::CssProvider::new();
    css.load_from_data(include_str!("photo60.css"));
    if let Some(display) = gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &css,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
    let window = ApplicationWindow::builder()
        .application(app)
        .title(concat!("60 Hz 实拍对照 · ", env!("CARGO_PKG_VERSION")))
        .default_width(996)
        .default_height(710)
        .build();
    window.add_css_class("photo60");
    let mut media = match ExperimentMedia::new() {
        Ok(media) => media,
        Err(err) => {
            window.set_child(Some(&Label::new(Some(&format!("素材读取失败：{err}")))));
            window.present();
            return;
        }
    };
    media.set_strength(Strength::Latent);
    let column = Row::new(Orientation::Vertical, 0);
    let header = Row::new(Orientation::Horizontal, 9);
    header.add_css_class("header");
    let title = Label::new(Some("60 Hz 实拍对照"));
    title.add_css_class("title");
    header.append(&title);
    let profiles = Row::new(Orientation::Horizontal, 0);
    profiles.add_css_class("profiles");
    profiles.add_css_class("linked");
    let standard_button = ToggleButton::with_label("标准（旧版）");
    standard_button.set_tooltip_text(Some("选择 0.1.0 的标准干扰（3）"));
    let strong_button = ToggleButton::with_label("增强（笔画）");
    strong_button.set_tooltip_text(Some("选择针对笔画边缘的增强干扰（4）；空格开启实验"));
    strong_button.set_group(Some(&standard_button));
    let latent_button = ToggleButton::with_label("翻拍暗字");
    latent_button.set_tooltip_text(Some("沿用 0.3 的交替色块（5）；空格开启实验"));
    latent_button.set_group(Some(&standard_button));
    latent_button.set_active(true);
    let balanced_button = ToggleButton::with_label("减闪暗字");
    balanced_button.set_tooltip_text(Some("降低相邻两帧的亮度变化（6）；空格开启实验"));
    balanced_button.set_group(Some(&standard_button));
    profiles.append(&standard_button);
    profiles.append(&strong_button);
    profiles.append(&latent_button);
    profiles.append(&balanced_button);
    header.append(&profiles);
    header.append(&spacer());
    let text_button = make_button("黑白文字", "显示黑白文字页（1）");
    let video_button = make_button("字幕视频", "播放带有中文字幕的无声短片（2）");
    let full = make_button("全屏", "切换全屏（F11）");
    header.append(&text_button);
    header.append(&video_button);
    header.append(&full);
    column.append(&header);

    let picture = Picture::new();
    picture.add_css_class("stage");
    picture.set_hexpand(true);
    picture.set_vexpand(true);
    picture.set_can_shrink(true);
    picture.set_content_fit(gtk4::ContentFit::Contain);
    column.append(&picture);

    let lower = Row::new(Orientation::Vertical, 0);
    lower.add_css_class("lower");
    let controls = Row::new(Orientation::Horizontal, 10);
    controls.add_css_class("controls");
    let mode = Label::new(None);
    mode.add_css_class("title");
    let rate = Label::new(Some("回调测量中…"));
    rate.add_css_class("stat");
    rate.set_tooltip_text(Some(
        "最近 30 次 GTK 帧回调的平均频率；显示器实际输出请在系统设置中核对。",
    ));
    let play_time = Label::new(Some("00.0 s"));
    play_time.add_css_class("stat");
    let pause_button = make_button("暂停视频", "暂停内容，实验模式仍会交替显示同一视频帧");
    let restart_button = make_button("回到片头", "回到同一素材的开头（R）");
    let experiment_button = make_button("开始实验", "在原画和实验之间切换（空格）；Esc 返回原画");
    experiment_button.add_css_class("action");
    controls.append(&mode);
    controls.append(&rate);
    controls.append(&spacer());
    controls.append(&play_time);
    controls.append(&pause_button);
    controls.append(&restart_button);
    controls.append(&experiment_button);
    let hints = Row::new(Orientation::Horizontal, 0);
    hints.add_css_class("hints");
    let hint = Label::new(Some(NORMAL_HINT));
    hint.add_css_class("hint");
    hint.set_xalign(0.0);
    hint.set_wrap(true);
    hints.append(&hint);
    lower.append(&controls);
    lower.append(&hints);
    column.append(&lower);
    window.set_child(Some(&column));

    let view = Rc::new(Viewer {
        window: window.downgrade(),
        picture,
        text_button,
        video_button,
        standard_button,
        strong_button,
        latent_button,
        balanced_button,
        experiment_button,
        pause_button,
        restart_button,
        rate,
        mode,
        play_time,
        hint,
        media: RefCell::new(media),
        state: RefCell::new(State::default()),
    });
    for (button, strength) in [
        (&view.standard_button, Strength::Standard),
        (&view.strong_button, Strength::Strong),
        (&view.latent_button, Strength::Latent),
        (&view.balanced_button, Strength::Balanced),
    ] {
        let weak = Rc::downgrade(&view);
        button.connect_toggled(move |button| {
            if button.is_active() {
                if let Some(viewer) = weak.upgrade() {
                    viewer.select_strength(strength);
                }
            }
        });
    }
    view.bind(&view.text_button, |v| v.select(Scene::Text));
    view.bind(&view.video_button, |v| v.select(Scene::Video));
    view.bind(&view.experiment_button, Viewer::toggle_mode);
    view.bind(&view.pause_button, Viewer::pause);
    view.bind(&view.restart_button, Viewer::restart);
    view.bind(&full, Viewer::fullscreen);
    let weak = Rc::downgrade(&view);
    let keys = gtk4::EventControllerKey::new();
    keys.set_propagation_phase(gtk4::PropagationPhase::Capture);
    keys.connect_key_pressed(move |_, key, _, modifiers| {
        let Some(v) = weak.upgrade() else {
            return glib::Propagation::Proceed;
        };
        if modifiers.intersects(
            gdk::ModifierType::CONTROL_MASK
                | gdk::ModifierType::ALT_MASK
                | gdk::ModifierType::SUPER_MASK,
        ) {
            return glib::Propagation::Proceed;
        }
        match key {
            gdk::Key::_1 => v.select(Scene::Text),
            gdk::Key::_2 => v.select(Scene::Video),
            gdk::Key::_3 => v.standard_button.set_active(true),
            gdk::Key::_4 => v.strong_button.set_active(true),
            gdk::Key::_5 => v.latent_button.set_active(true),
            gdk::Key::_6 => v.balanced_button.set_active(true),
            gdk::Key::space => v.toggle_mode(),
            gdk::Key::F11 => v.fullscreen(),
            gdk::Key::r | gdk::Key::R => v.restart(),
            gdk::Key::Escape => {
                if v.state.borrow().experiment {
                    v.original(NORMAL_HINT)
                } else if let Some(win) = v.window.upgrade() {
                    win.unfullscreen()
                }
            }
            _ => return glib::Propagation::Proceed,
        }
        glib::Propagation::Stop
    });
    window.add_controller(keys);
    let weak = Rc::downgrade(&view);
    window.connect_is_active_notify(move |win| {
        if !win.is_active() {
            if let Some(v) = weak.upgrade() {
                if v.state.borrow().experiment {
                    v.original(NORMAL_HINT)
                }
            }
        }
    });
    view.controls();
    view.draw();
    window.add_tick_callback(move |_, clock| {
        view.tick(clock.frame_time());
        glib::ControlFlow::Continue
    });
    window.present();
}

fn main() -> glib::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [arg] if arg == "--version" || arg == "-V" => {
            println!("photo60-test {}", env!("CARGO_PKG_VERSION"));
            return glib::ExitCode::SUCCESS;
        }
        [arg] if arg == "--help" || arg == "-h" => {
            println!("photo60-test — 60 Hz 实拍对照\n1 黑白文字；2 字幕视频；3 标准档；4 增强档；5 翻拍暗字；6 减闪暗字；空格 原画 / 实验；Esc 返回原画；F11 全屏；R 回到片头。");
            return glib::ExitCode::SUCCESS;
        }
        [] => {}
        _ => {
            eprintln!("用法：photo60-test [--version | --help]");
            return glib::ExitCode::FAILURE;
        }
    }
    let app = Application::new(Some(APP_ID), gio::ApplicationFlags::empty());
    app.connect_activate(build);
    app.run()
}
