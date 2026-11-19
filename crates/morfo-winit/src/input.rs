//! Turning winit's events into what morfo understands: the pointer, the
//! wheel, keys, text, input methods, the clipboard and the standard
//! Ctrl shortcuts.

use morfo::{Key, Modifiers, Ui, Vec2};
use winit::event::{ElementState, Ime, MouseButton, MouseScrollDelta, Touch, TouchPhase, WindowEvent};
use winit::keyboard::{Key as WinitKey, NamedKey};
use winit::window::Window;

/// Logical pixels one line of a wheel notch scrolls.
const LINE: f32 = 48.0;

/// What the host remembers between events.
pub struct Input {
    mods: Modifiers,
    #[cfg(feature = "clipboard")]
    clipboard: Option<arboard::Clipboard>,
    /// The touch that drives the pointer (one finger at a time).
    finger: Option<u64>,
    /// Every finger down, where it is (two of them pinch).
    fingers: Vec<(u64, Vec2)>,
}

impl Default for Input {
    fn default() -> Self {
        Input {
            mods: Modifiers::default(),
            #[cfg(feature = "clipboard")]
            clipboard: arboard::Clipboard::new().ok(),
            finger: None,
            fingers: Vec::new(),
        }
    }
}

/// A Ctrl shortcut the application may want for itself (Ctrl+K…), passed
/// back by [`Input::handle`] when no field claimed it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chord(pub String);

/// What an event meant for the host.
#[derive(Default)]
pub struct Handled {
    /// The interface must be built again.
    pub redraw: bool,
    /// A Ctrl+key that is not one of the editing shortcuts.
    pub chord: Option<Chord>,
}

impl Input {
    /// Hands `event` to `ui`. Ctrl+A/C/X/V edit text; any other Ctrl+key
    /// goes to a shortcut field that is recording, or back to the caller.
    pub fn handle(&mut self, ui: &mut Ui, window: &Window, event: &WindowEvent) -> Handled {
        let scale = window.scale_factor();
        let mut out = Handled { redraw: true, chord: None };
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                let p = position.to_logical::<f32>(scale);
                ui.pointer_moved(Vec2::new(p.x, p.y));
            }
            WindowEvent::CursorLeft { .. } => ui.pointer_left(),
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => match state {
                ElementState::Pressed => ui.pointer_pressed(),
                ElementState::Released => ui.pointer_released(),
            },
            WindowEvent::Touch(touch) => self.touch(ui, touch, scale),
            WindowEvent::ModifiersChanged(m) => {
                self.mods = Modifiers { shift: m.state().shift_key(), ctrl: m.state().control_key() || m.state().super_key() };
                ui.set_modifiers(self.mods);
                out.redraw = false;
            }
            WindowEvent::MouseWheel { delta, .. } => ui.scroll_wheel(match delta {
                MouseScrollDelta::LineDelta(_, lines) => lines * LINE,
                MouseScrollDelta::PixelDelta(p) => (p.y / scale) as f32,
            }),
            WindowEvent::Ime(Ime::Preedit(text, _)) => ui.ime_preedit(text),
            WindowEvent::Ime(Ime::Commit(text)) => {
                ui.ime_preedit("");
                ui.text_input(text);
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                let key = key_of(&event.logical_key);
                if self.mods.ctrl && ui.wants_keys() {
                    // A shortcut field is recording: the combination is its.
                    match (&event.logical_key, key) {
                        (WinitKey::Character(c), _) => ui.chord(c),
                        (_, Some(key)) => ui.key_pressed(key),
                        _ => {}
                    }
                } else if self.mods.ctrl {
                    // Text typed with Ctrl held is a shortcut, not text.
                    if let WinitKey::Character(c) = &event.logical_key {
                        if !self.edit(ui, &c.to_lowercase()) {
                            out.chord = Some(Chord(c.to_lowercase()));
                        }
                    } else if let Some(key) = key {
                        ui.key_pressed(key);
                    }
                } else {
                    if let Some(key) = key {
                        ui.key_pressed(key);
                    }
                    // Space activates controls and is also text.
                    if key.is_none() || key == Some(Key::Space) {
                        if let Some(text) = &event.text {
                            ui.text_input(text);
                        }
                    }
                }
            }
            _ => out.redraw = false,
        }
        out
    }

    /// One finger is the pointer: down presses, moving drags, up releases.
    /// Two fingers pinch.
    fn touch(&mut self, ui: &mut Ui, touch: &Touch, scale: f64) {
        let p = touch.location.to_logical::<f32>(scale);
        let at = Vec2::new(p.x, p.y);
        let before = self.pair();
        match touch.phase {
            TouchPhase::Started | TouchPhase::Moved => match self.fingers.iter_mut().find(|(id, _)| *id == touch.id) {
                Some(f) => f.1 = at,
                None => self.fingers.push((touch.id, at)),
            },
            TouchPhase::Ended | TouchPhase::Cancelled => self.fingers.retain(|(id, _)| *id != touch.id),
        }
        if let (Some((a0, b0)), Some((a1, b1))) = (before, self.pair()) {
            let (d0, d1) = ((b0 - a0).length(), (b1 - a1).length());
            if d0 > 1.0 {
                ui.pinch_moved(d1 / d0, (a1 + b1) * 0.5);
            }
        }
        match touch.phase {
            TouchPhase::Started if self.finger.is_none() => {
                self.finger = Some(touch.id);
                ui.touch_started(at);
            }
            TouchPhase::Moved if self.finger == Some(touch.id) => ui.touch_moved(at),
            TouchPhase::Ended | TouchPhase::Cancelled if self.finger == Some(touch.id) => {
                self.finger = None;
                ui.touch_ended(at, touch.phase == TouchPhase::Cancelled);
            }
            _ => {}
        }
    }

    /// The first two fingers down, if there are two.
    fn pair(&self) -> Option<(Vec2, Vec2)> {
        (self.fingers.len() >= 2).then(|| (self.fingers[0].1, self.fingers[1].1))
    }

    /// The editing shortcuts; whether `c` was one.
    fn edit(&mut self, ui: &mut Ui, c: &str) -> bool {
        match c {
            "a" => ui.key_pressed(Key::SelectAll),
            "c" => ui.key_pressed(Key::Copy),
            "x" => ui.key_pressed(Key::Cut),
            "z" if self.mods.shift => ui.key_pressed(Key::Redo),
            "z" => ui.key_pressed(Key::Undo),
            "y" => ui.key_pressed(Key::Redo),
            "v" =>
            {
                #[cfg(feature = "clipboard")]
                if let Some(text) = self.clipboard.as_mut().and_then(|c| c.get_text().ok()) {
                    ui.paste(&text);
                }
            }
            _ => return false,
        }
        true
    }

    /// After a frame: what was copied or cut goes to the system clipboard,
    /// and the input method's window follows the caret.
    pub fn after_frame(&mut self, ui: &mut Ui, window: &Window) {
        #[cfg(feature = "clipboard")]
        if let (Some(text), Some(clipboard)) = (ui.take_clipboard(), self.clipboard.as_mut()) {
            let _ = clipboard.set_text(text);
        }
        #[cfg(not(feature = "clipboard"))]
        let _ = ui.take_clipboard();
        if let Some(r) = ui.ime_area() {
            window.set_ime_cursor_area(winit::dpi::LogicalPosition::new(r.x, r.y), winit::dpi::LogicalSize::new(r.w, r.h));
        }
    }
}

/// The morfo key for a winit key, if it is one morfo uses.
pub fn key_of(key: &WinitKey) -> Option<Key> {
    let WinitKey::Named(named) = key else { return None };
    Some(match named {
        NamedKey::Backspace => Key::Backspace,
        NamedKey::Delete => Key::Delete,
        NamedKey::ArrowLeft => Key::Left,
        NamedKey::ArrowRight => Key::Right,
        NamedKey::ArrowUp => Key::Up,
        NamedKey::ArrowDown => Key::Down,
        NamedKey::Home => Key::Home,
        NamedKey::End => Key::End,
        NamedKey::PageUp => Key::PageUp,
        NamedKey::PageDown => Key::PageDown,
        NamedKey::Enter => Key::Enter,
        NamedKey::Escape => Key::Escape,
        NamedKey::Tab => Key::Tab,
        NamedKey::Space => Key::Space,
        _ => return None,
    })
}
