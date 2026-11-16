use morfo::{Color, DrawCmd, Theme, Ui, Vec2};

const VIEW: Vec2 = Vec2::new(800.0, 600.0);
const ITEMS: [(&str, &str); 3] = [("a", "Alpha"), ("b", "Beta"), ("c", "Gamma")];

fn frame(ui: &mut Ui, t: &mut f64, mut f: impl FnMut(&mut Ui)) -> bool {
    *t += 1.0 / 60.0;
    ui.begin(*t, VIEW);
    f(ui);
    ui.end().animating
}

fn settle(ui: &mut Ui, t: &mut f64, mut f: impl FnMut(&mut Ui)) {
    for _ in 0..1000 {
        if !frame(ui, t, &mut f) {
            return;
        }
    }
    panic!("never came to rest");
}

fn click(ui: &mut Ui, t: &mut f64, id: morfo::Id, mut f: impl FnMut(&mut Ui)) {
    let p = ui.rect_of(id).unwrap().center();
    ui.pointer_moved(p);
    frame(ui, t, &mut f);
    ui.pointer_pressed();
    frame(ui, t, &mut f);
    ui.pointer_released();
    frame(ui, t, &mut f);
}

#[test]
fn rests_and_wakes() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let mut on = false;
    settle(&mut ui, &mut t, |ui| {
        ui.toggle("sw", &mut on);
    });
    // At rest: every further frame reports idle.
    for _ in 0..10 {
        assert!(!frame(&mut ui, &mut t, |ui| {
            ui.toggle("sw", &mut on);
        }));
    }
    // A click flips the value and wakes the animation up.
    let id = ui.id("sw");
    click(&mut ui, &mut t, id, |ui| {
        ui.toggle("sw", &mut on);
    });
    assert!(on);
    assert!(frame(&mut ui, &mut t, |ui| {
        ui.toggle("sw", &mut on);
    }));
}

#[test]
fn layout_changes_are_continuous() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let mut open = false;
    let build = |ui: &mut Ui, open: bool| {
        ui.reveal("extra", open, |ui| {
            ui.label("one");
            ui.label("two");
        });
        ui.button("below", "Below");
    };
    settle(&mut ui, &mut t, |ui| build(ui, open));
    let id = ui.id("below");
    let y0 = ui.rect_of(id).unwrap().y;

    open = true;
    let mut last = y0;
    let mut max_step = 0.0f32;
    for _ in 0..600 {
        let moving = frame(&mut ui, &mut t, |ui| build(ui, open));
        let y = ui.rect_of(id).unwrap().y;
        max_step = max_step.max((y - last).abs());
        last = y;
        if !moving {
            break;
        }
    }
    assert!(last > y0 + 30.0, "the button was pushed down");
    assert!(max_step < 8.0, "it moved smoothly, never jumped ({max_step} px in one frame)");
}

#[test]
fn toolbar_adapts_to_usage() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let build = |ui: &mut Ui| {
        ui.adaptive_actions("bar", &ITEMS);
    };
    settle(&mut ui, &mut t, build);
    let id = |ui: &Ui, k: &str| ui.id("bar").child(k);
    let (a, c) = (id(&ui, "a"), id(&ui, "c"));
    assert!(ui.rect_of(a).unwrap().x < ui.rect_of(c).unwrap().x);
    let width_before = ui.rect_of(c).unwrap().w;

    for _ in 0..6 {
        click(&mut ui, &mut t, c, build);
    }
    ui.pointer_left();
    settle(&mut ui, &mut t, build);
    // The heavily used action moved to the front and grew.
    assert!(ui.rect_of(c).unwrap().x < ui.rect_of(a).unwrap().x);
    assert!(ui.rect_of(c).unwrap().w > width_before);

    // A different user (fresh profile) gets the original interface back.
    ui.set_profile(morfo::Profile::new());
    settle(&mut ui, &mut t, build);
    assert!(ui.rect_of(a).unwrap().x < ui.rect_of(c).unwrap().x);
}

#[test]
fn steady_frames_do_not_grow_the_draw_list() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let mut v = 0.5;
    let mut build = |ui: &mut Ui| {
        ui.card("c", |ui| {
            ui.heading("Title");
            ui.slider("s", &mut v, 200.0);
            ui.button("b", "Go");
        });
    };
    settle(&mut ui, &mut t, &mut build);
    let n = ui.draw_list().len();
    assert!(ui.draw_list().commands().iter().any(|c| matches!(c, DrawCmd::Text { .. })));
    frame(&mut ui, &mut t, &mut build);
    assert_eq!(ui.draw_list().len(), n);
}

// ----- floating, selection, keyboard and scroll ---------------------------

use morfo::Key;

#[test]
fn dialog_blocks_what_is_under_it_and_animates_out() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let (mut open, mut under, mut inside) = (false, 0, 0);
    let build = |ui: &mut Ui, open: &mut bool, under: &mut u32, inside: &mut u32| {
        if ui.button("trigger", "Open").clicked {
            *open = true;
        }
        if ui.button("under", "Under").clicked {
            *under += 1;
        }
        ui.dialog("dlg", "trigger", open, 300.0, |ui| {
            ui.label("Hello");
            if ui.button("ok", "OK").clicked {
                *inside += 1;
                ui.close();
            }
        });
    };
    macro_rules! go {
        () => {
            |ui: &mut Ui| build(ui, &mut open, &mut under, &mut inside)
        };
    }
    settle(&mut ui, &mut t, go!());
    let (trigger, below, ok) = (ui.id("trigger"), ui.id("under"), ui.id("ok"));
    assert!(ui.rect_of(ok).is_none(), "closed dialogs are not built");
    click(&mut ui, &mut t, trigger, go!());
    settle(&mut ui, &mut t, go!());
    assert!(open);
    // The button underneath no longer reacts...
    click(&mut ui, &mut t, below, go!());
    assert_eq!(under, 0);
    // ...and that click outside the panel dismissed the dialog.
    assert!(!open);
    settle(&mut ui, &mut t, go!());

    click(&mut ui, &mut t, trigger, go!());
    settle(&mut ui, &mut t, go!());
    click(&mut ui, &mut t, ok, go!());
    assert_eq!(inside, 1);
    assert!(!open);
    // Still being drawn while it shrinks back: an exit animation.
    assert!(frame(&mut ui, &mut t, go!()));
    settle(&mut ui, &mut t, go!());
    // Once closed, what was under it works again.
    click(&mut ui, &mut t, below, go!());
    assert_eq!(under, 1);

    // Escape closes too.
    click(&mut ui, &mut t, trigger, go!());
    ui.key_pressed(Key::Escape);
    frame(&mut ui, &mut t, go!());
    assert!(!open);
}

#[test]
fn menu_picks_an_option_and_closes() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let mut picked = None;
    let items = [("cut", "Cut"), ("copy", "Copy"), ("paste", "Paste")];
    let mut build = |ui: &mut Ui| {
        if let Some(i) = ui.menu("edit", "Edit", &items) {
            picked = Some(i);
        }
    };
    settle(&mut ui, &mut t, &mut build);
    let menu = ui.id("edit");
    assert!(ui.rect_of(menu.child("copy")).is_none());
    click(&mut ui, &mut t, menu, &mut build);
    settle(&mut ui, &mut t, &mut build);
    click(&mut ui, &mut t, menu.child("copy"), &mut build);
    settle(&mut ui, &mut t, &mut build);
    assert_eq!(picked, Some(1));
}

#[test]
fn toasts_leave_by_themselves() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    settle(&mut ui, &mut t, |_| {});
    let empty = ui.draw_list().len();
    ui.toast("Saved");
    settle(&mut ui, &mut t, |_| {});
    assert!(ui.draw_list().len() > empty);
    // At rest, but it asks to be woken when the toast expires.
    t += 1.0 / 60.0;
    ui.begin(t, VIEW);
    let wake = ui.end().wake_in.expect("a wake-up is scheduled");
    assert!(wake > 1.0 && wake < 3.5);
    t += wake as f64;
    settle(&mut ui, &mut t, |_| {});
    assert_eq!(ui.draw_list().len(), empty);
    ui.begin(t + 1.0, VIEW);
    assert!(ui.end().wake_in.is_none());
}

#[test]
fn selection_controls() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let (mut check, mut choice, mut chip, mut n, mut section) = (false, 0usize, false, 9, None);
    let mut build = |ui: &mut Ui| {
        ui.checkbox("check", "Check", &mut check);
        ui.radio("r0", "Zero", &mut choice, 0);
        ui.radio("r1", "One", &mut choice, 1);
        ui.chip("chip", "Chip", &mut chip);
        ui.stepper("n", &mut n, 0, 10);
        ui.accordion("acc", &["A", "B"], &mut section, |ui, i| {
            ui.button(if i == 0 { "in-a" } else { "in-b" }, "Inside");
        });
    };
    settle(&mut ui, &mut t, &mut build);
    for key in ["check", "r1", "chip"] {
        let id = ui.id(key);
        click(&mut ui, &mut t, id, &mut build);
    }
    // Stepper: + twice from 9 stops at the maximum.
    let plus = ui.id("n").with(1);
    click(&mut ui, &mut t, plus, &mut build);
    click(&mut ui, &mut t, plus, &mut build);
    // Accordion: opening B builds its body; opening A closes B.
    let acc = ui.id("acc");
    click(&mut ui, &mut t, acc.with(1), &mut build);
    settle(&mut ui, &mut t, &mut build);
    let in_b = ui.id("in-b");
    assert!(ui.rect_of(in_b).is_some());
    click(&mut ui, &mut t, acc.with(0), &mut build);
    settle(&mut ui, &mut t, &mut build);
    drop(build);
    assert!(check && chip);
    assert_eq!((choice, n, section), (1, 10, Some(0)));
}

#[test]
fn text_field_editing_and_focus() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let (mut a, mut b) = (String::new(), String::from("xy"));
    let mut submitted = false;
    let mut build = |ui: &mut Ui| {
        submitted |= ui.text_field("a", &mut a, "Name", 200.0).submitted;
        ui.text_field("b", &mut b, "Other", 200.0);
    };
    settle(&mut ui, &mut t, &mut build);
    // Typing with no focus goes nowhere.
    ui.text_input("zzz");
    frame(&mut ui, &mut t, &mut build);
    let field = ui.id("a");
    click(&mut ui, &mut t, field, &mut build);
    assert!(ui.has_focus());
    ui.text_input("Año");
    frame(&mut ui, &mut t, &mut build);
    // Caret keys respect multi-byte characters.
    for key in [Key::Left, Key::Backspace, Key::Home, Key::Delete, Key::End] {
        ui.key_pressed(key);
    }
    frame(&mut ui, &mut t, &mut build);
    ui.text_input("!");
    ui.key_pressed(Key::Enter);
    frame(&mut ui, &mut t, &mut build);
    // Tab moves to the next field, with the caret at its end.
    ui.key_pressed(Key::Tab);
    frame(&mut ui, &mut t, &mut build);
    ui.text_input("z");
    frame(&mut ui, &mut t, &mut build);
    // A focused field keeps asking for frames to blink its caret.
    t += 1.0 / 60.0;
    ui.begin(t, VIEW);
    build(&mut ui);
    assert!(ui.end().wake_in.is_some());
    ui.key_pressed(Key::Escape);
    frame(&mut ui, &mut t, &mut build);
    assert!(!ui.has_focus());
    drop(build);
    assert_eq!(a, "o!");
    assert_eq!(b, "xyz");
    assert!(submitted);
}

#[test]
fn scroll_clamps_and_culls() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let build = |ui: &mut Ui| {
        ui.scroll("list", 120.0, |ui| {
            for _ in 0..50 {
                ui.label("row");
            }
        });
        ui.button("after", "After");
    };
    settle(&mut ui, &mut t, build);
    // Only the rows inside the viewport are drawn.
    let texts = |ui: &Ui| ui.draw_list().commands().iter().filter(|c| matches!(c, DrawCmd::Text { .. })).count();
    assert!(texts(&ui) < 12, "{} rows drawn", texts(&ui));
    // What follows is placed after the viewport, not after the content.
    let after = ui.id("after");
    let y = ui.rect_of(after).unwrap().y;
    assert!(y < 220.0);

    let first_text_y =
        |ui: &Ui| ui.draw_list().commands().iter().find_map(|c| if let DrawCmd::Text { pos, .. } = c { Some(pos.y) } else { None }).unwrap();
    let top = first_text_y(&ui);
    ui.pointer_moved(Vec2::new(60.0, 60.0));
    ui.scroll_wheel(-100_000.0);
    settle(&mut ui, &mut t, build);
    assert!(first_text_y(&ui) != top && texts(&ui) > 0);
    assert_eq!(ui.rect_of(after).unwrap().y, y, "scrolling moves nothing outside");
    // Scrolling back past the start stops at the start.
    ui.scroll_wheel(100_000.0);
    settle(&mut ui, &mut t, build);
    assert_eq!(first_text_y(&ui), top);
}

#[test]
fn suggestions_follow_habits() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let build = |ui: &mut Ui| {
        ui.adaptive_actions("bar", &ITEMS);
        ui.suggestions("quick", "Quick:", "bar", &ITEMS, 2);
    };
    settle(&mut ui, &mut t, build);
    let shortcut = ui.id("quick").child("b");
    assert!(ui.rect_of(shortcut).is_none(), "no habits, no suggestions");
    let b = ui.id("bar").child("b");
    for _ in 0..3 {
        click(&mut ui, &mut t, b, build);
    }
    ui.pointer_left();
    settle(&mut ui, &mut t, build);
    assert!(ui.rect_of(shortcut).is_some());
    assert!(ui.rect_of(ui.id("quick").child("a")).is_none());
}

// ----- mutation -------------------------------------------------------------

fn texts(ui: &Ui) -> Vec<String> {
    let list = ui.draw_list();
    list.commands()
        .iter()
        .filter_map(|c| if let DrawCmd::Text { start, len, .. } = *c { Some(list.text(start, len).to_string()) } else { None })
        .collect()
}

#[test]
fn a_component_mutates_into_another() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let build = |ui: &mut Ui, form: u8| {
        ui.morph("m", form, |ui| match form {
            0 => {
                ui.button("b", "Short button");
            }
            _ => ui.card_w("c", 400.0, |ui| {
                ui.title("Big card");
                ui.label("with content");
            }),
        });
        ui.button("below", "Below");
    };
    settle(&mut ui, &mut t, |ui| build(ui, 0));
    let (m, below) = (ui.id("m"), ui.id("below"));
    let small = ui.rect_of(m).unwrap();
    let y0 = ui.rect_of(below).unwrap().y;
    assert!(texts(&ui).contains(&"Short button".to_string()));

    // A few frames in, both forms are on screen and the shape is in between.
    for _ in 0..8 {
        assert!(frame(&mut ui, &mut t, |ui| build(ui, 1)));
    }
    let mid = ui.rect_of(m).unwrap();
    let now = texts(&ui);
    assert!(now.contains(&"Short button".to_string()) && now.contains(&"Big card".to_string()), "{now:?}");
    assert!(mid.w > small.w + 5.0 && mid.w < 395.0, "{mid:?}");

    // What follows makes room smoothly, and it all settles in the new form.
    let mut last = ui.rect_of(below).unwrap().y;
    let mut max_step = 0.0f32;
    for _ in 0..600 {
        let moving = frame(&mut ui, &mut t, |ui| build(ui, 1));
        let y = ui.rect_of(below).unwrap().y;
        max_step = max_step.max((y - last).abs());
        last = y;
        if !moving {
            break;
        }
    }
    assert!(last > y0 + 20.0);
    assert!(max_step < 12.0, "{max_step} px in one frame");
    assert!(!texts(&ui).contains(&"Short button".to_string()));
    assert_eq!(ui.rect_of(m).unwrap().w, 400.0);
    // Keys inside are namespaced under the morph's key.
    assert!(ui.rect_of(m.child("c")).is_some());

    // Back again, and at rest nothing is drawn twice.
    settle(&mut ui, &mut t, |ui| build(ui, 0));
    assert_eq!(texts(&ui).iter().filter(|s| *s == "Short button").count(), 1);
    assert!(!texts(&ui).contains(&"Big card".to_string()));
}

#[test]
fn a_button_becomes_a_focused_text_field() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let (mut open, mut query) = (false, String::new());
    let mut build = |ui: &mut Ui| {
        ui.morph("search", open, |ui| {
            if !open {
                if ui.button("open", "Search").clicked {
                    open = true;
                    ui.request_focus("field");
                }
            } else if !ui.text_field("field", &mut query, "Type", 300.0).focused && query.is_empty() {
                open = false;
            }
        });
    };
    settle(&mut ui, &mut t, &mut build);
    let button = ui.id("search").child("open");
    click(&mut ui, &mut t, button, &mut build);
    frame(&mut ui, &mut t, &mut build);
    assert!(ui.has_focus(), "the field arrives with the focus");
    ui.text_input("hi");
    settle(&mut ui, &mut t, &mut build);
    // Leaving it empty and unfocused turns it back into the button.
    for _ in 0..2 {
        ui.key_pressed(Key::Backspace);
    }
    ui.key_pressed(Key::Escape);
    settle(&mut ui, &mut t, &mut build);
    drop(build);
    assert!(!open);
}

#[test]
fn clicking_elsewhere_drops_the_focus() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let mut text = String::new();
    let mut build = |ui: &mut Ui| {
        ui.text_field("f", &mut text, "Name", 200.0);
        ui.button("other", "Other");
    };
    settle(&mut ui, &mut t, &mut build);
    let (field, other) = (ui.id("f"), ui.id("other"));
    click(&mut ui, &mut t, field, &mut build);
    assert!(ui.has_focus());
    click(&mut ui, &mut t, other, &mut build);
    assert!(!ui.has_focus());
}

#[test]
fn side_nav_unfolds_from_the_edge() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let (mut page, mut under) = (0usize, 0);
    let mut build = |ui: &mut Ui| {
        ui.side_nav("nav", "Docs", &["One", "Two", "Three"], &mut page);
        if ui.button("content", "Content").clicked {
            under += 1;
        }
    };
    settle(&mut ui, &mut t, &mut build);
    let row = ui.id("nav").with(2);
    // Collapsed, each entry is a dot on the rail.
    assert!(ui.rect_of(row).unwrap().w < 10.0);

    // Bringing the pointer to the left edge unfolds it.
    ui.pointer_moved(Vec2::new(3.0, 300.0));
    settle(&mut ui, &mut t, &mut build);
    assert!(ui.rect_of(row).unwrap().w > 150.0);

    // Choosing an entry selects it and folds the index, pointer or not.
    click(&mut ui, &mut t, row, &mut build);
    settle(&mut ui, &mut t, &mut build);
    assert!(ui.rect_of(row).unwrap().w < 10.0);

    // While open, what is underneath does not react.
    ui.pointer_moved(Vec2::new(3.0, 300.0));
    settle(&mut ui, &mut t, &mut build);
    let content = ui.id("content");
    click(&mut ui, &mut t, content, &mut build);

    // Leaving it, it lingers a moment (asking to be woken), then folds.
    ui.pointer_moved(Vec2::new(600.0, 300.0));
    frame(&mut ui, &mut t, &mut build);
    assert!(ui.rect_of(row).unwrap().w > 150.0);
    t += 1.0 / 60.0;
    ui.begin(t, VIEW);
    build(&mut ui);
    assert!(ui.end().wake_in.is_some());
    t += 0.5;
    settle(&mut ui, &mut t, &mut build);
    assert!(ui.rect_of(row).unwrap().w < 10.0);
    drop(build);
    assert_eq!((page, under), (2, 0));
}

// ----- button kinds -------------------------------------------------------

use morfo::{Icon, Size, Variant};

#[test]
fn every_button_kind_in_every_variant_and_size() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let mut on = [false; 12];
    let build = |ui: &mut Ui, on: &mut [bool; 12]| {
        let mut i = 0;
        for size in Size::ALL {
            for variant in Variant::ALL {
                let k = |kind: &str| format!("{kind}-{i}");
                ui.button_with(&k("plain"), "Label", variant, size);
                ui.icon_button(&k("icon"), Icon::Star, variant, size);
                ui.button_icon(&k("both"), Icon::Download, "Label", variant, size);
                ui.toggle_button(&k("toggle"), "Label", &mut on[i], variant, size);
                ui.loading_button(&k("loading"), "Label", false, variant, size);
                ui.hold_button(&k("hold"), "Label", 1.0, variant, size);
                i += 1;
            }
        }
    };
    settle(&mut ui, &mut t, |ui| build(ui, &mut on));
    for i in 0..12 {
        let w = |kind: &str| ui.rect_of(ui.id(&format!("{kind}-{i}"))).unwrap();
        let (plain, icon, both) = (w("plain"), w("icon"), w("both"));
        assert_eq!(icon.w, icon.h, "icon buttons are round");
        assert!(both.w > plain.w, "the icon adds room");
        assert_eq!(w("loading").w, plain.w);
        assert_eq!(w("hold").w, plain.w);
    }
}

#[test]
fn toggle_button_stays_pressed_and_grows_a_check() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let mut on = false;
    let mut build = |ui: &mut Ui| {
        ui.toggle_button("follow", "Follow", &mut on, Variant::Outline, Size::Md);
    };
    settle(&mut ui, &mut t, &mut build);
    let id = ui.id("follow");
    let before = ui.rect_of(id).unwrap().w;
    click(&mut ui, &mut t, id, &mut build);
    ui.pointer_left();
    settle(&mut ui, &mut t, &mut build);
    assert!(ui.rect_of(id).unwrap().w > before);
    click(&mut ui, &mut t, id, &mut build);
    ui.pointer_left();
    settle(&mut ui, &mut t, &mut build);
    assert_eq!(ui.rect_of(id).unwrap().w, before);
    drop(build);
    assert!(!on);
}

#[test]
fn loading_button_turns_into_a_spinner_and_ignores_clicks() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let mut clicks = 0;
    settle(&mut ui, &mut t, |ui| {
        ui.loading_button("send", "Send", false, Variant::Solid, Size::Md);
    });
    let id = ui.id("send");
    let wide = ui.rect_of(id).unwrap();
    let mut build = |ui: &mut Ui| {
        if ui.loading_button("send", "Send", true, Variant::Solid, Size::Md).clicked {
            clicks += 1;
        }
    };
    for _ in 0..60 {
        // The spinner keeps turning: never at rest while busy.
        assert!(frame(&mut ui, &mut t, &mut build));
    }
    let round = ui.rect_of(id).unwrap();
    assert!((round.w - round.h).abs() < 1.0 && round.w < wide.w);
    click(&mut ui, &mut t, id, &mut build);
    drop(build);
    assert_eq!(clicks, 0);
}

#[test]
fn hold_button_fires_only_after_the_full_hold() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let mut fired = 0;
    let mut build = |ui: &mut Ui| {
        if ui.hold_button("delete", "Delete", 1.0, Variant::Outline, Size::Md) {
            fired += 1;
        }
    };
    settle(&mut ui, &mut t, &mut build);
    let id = ui.id("delete");
    ui.pointer_moved(ui.rect_of(id).unwrap().center());
    frame(&mut ui, &mut t, &mut build);
    // Let go after half a second: nothing happens.
    ui.pointer_pressed();
    for _ in 0..30 {
        frame(&mut ui, &mut t, &mut build);
    }
    ui.pointer_released();
    settle(&mut ui, &mut t, &mut build);
    // Hold for longer than a second: it fires, once.
    ui.pointer_pressed();
    for _ in 0..90 {
        frame(&mut ui, &mut t, &mut build);
    }
    ui.pointer_released();
    settle(&mut ui, &mut t, &mut build);
    drop(build);
    assert_eq!(fired, 1);
}

// ----- window chrome --------------------------------------------------------

use morfo::{ResizeEdge, TitleBar};

/// One frame with a press at `p`, returning what the bar reported.
fn press_on_bar(ui: &mut Ui, t: &mut f64, p: Vec2, maximized: bool) -> TitleBar {
    let mut out = TitleBar::default();
    ui.pointer_moved(p);
    frame(ui, t, |ui| {
        ui.title_bar("bar", "App", maximized, |_| {});
    });
    ui.pointer_pressed();
    frame(ui, t, |ui| {
        out = ui.title_bar("bar", "App", maximized, |_| {});
    });
    ui.pointer_released();
    frame(ui, t, |ui| {
        let r = ui.title_bar("bar", "App", maximized, |_| {});
        out.close |= r.close;
        out.minimize |= r.minimize;
        out.toggle_maximize |= r.toggle_maximize;
    });
    out
}

#[test]
fn title_bar_reports_window_actions() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let build = |ui: &mut Ui| {
        ui.title_bar("bar", "App", false, |ui| {
            ui.button("extra", "Extra");
        });
        ui.button("content", "Content");
    };
    settle(&mut ui, &mut t, build);
    // The layout starts below the bar.
    let h = ui.top_inset();
    assert!(h > 30.0);
    assert!(ui.rect_of(ui.id("content")).unwrap().y > h);
    // Content of the bar sits inside it.
    let extra = ui.rect_of(ui.id("bar").child("extra")).unwrap();
    assert!(extra.y >= 0.0 && extra.bottom() <= h);

    let close = ui.rect_of(ui.id("bar").child("close")).unwrap().center();
    assert!(press_on_bar(&mut ui, &mut t, close, false).close);
    let min = ui.rect_of(ui.id("bar").child("minimize")).unwrap().center();
    assert!(press_on_bar(&mut ui, &mut t, min, false).minimize);
    let max = ui.rect_of(ui.id("bar").child("maximize")).unwrap().center();
    assert!(press_on_bar(&mut ui, &mut t, max, false).toggle_maximize);

    // An empty part of the bar moves the window; a quick second press
    // maximizes it instead.
    let empty = Vec2::new(400.0, h * 0.5);
    let first = press_on_bar(&mut ui, &mut t, empty, false);
    assert!(first.drag && !first.toggle_maximize);
    let second = press_on_bar(&mut ui, &mut t, empty, false);
    assert!(second.toggle_maximize && !second.drag);
    t += 1.0;
    assert!(press_on_bar(&mut ui, &mut t, empty, false).drag);
}

#[test]
fn window_borders_resize_unless_maximized() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    settle(&mut ui, &mut t, |ui| {
        ui.title_bar("bar", "App", false, |_| {});
    });
    let r = press_on_bar(&mut ui, &mut t, Vec2::new(VIEW.x - 2.0, 300.0), false);
    assert_eq!(r.resize, Some(ResizeEdge::East));
    let r = press_on_bar(&mut ui, &mut t, Vec2::new(VIEW.x - 3.0, VIEW.y - 3.0), false);
    assert_eq!(r.resize, Some(ResizeEdge::SouthEast));
    let r = press_on_bar(&mut ui, &mut t, Vec2::new(VIEW.x - 2.0, 300.0), true);
    assert_eq!(r.resize, None);
    assert_eq!(r.hover_edge, None);
}

// ----- more controls ----------------------------------------------------------

/// Presses at `from`, drags to `to` over a few frames and releases.
fn drag(ui: &mut Ui, t: &mut f64, from: Vec2, to: Vec2, mut f: impl FnMut(&mut Ui)) {
    ui.pointer_moved(from);
    frame(ui, t, &mut f);
    ui.pointer_pressed();
    frame(ui, t, &mut f);
    for i in 1..=6 {
        ui.pointer_moved(from + (to - from) * (i as f32 / 6.0));
        frame(ui, t, &mut f);
    }
    ui.pointer_released();
    frame(ui, t, &mut f);
}

#[test]
fn range_slider_moves_the_nearest_thumb_and_keeps_order() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let (mut lo, mut hi) = (0.2f32, 0.6f32);
    let mut build = |ui: &mut Ui| {
        ui.range_slider("r", &mut lo, &mut hi, 400.0);
    };
    settle(&mut ui, &mut t, &mut build);
    let r = ui.rect_of(ui.id("r")).unwrap();
    let x = |v: f32| r.x + 12.0 + (r.w - 24.0) * v;
    // Grab near the upper thumb and drag it right.
    drag(&mut ui, &mut t, Vec2::new(x(0.62), r.center().y), Vec2::new(x(0.9), r.center().y), &mut build);
    // Grab the lower one and try to push it past the upper one: it stops there.
    drag(&mut ui, &mut t, Vec2::new(x(0.2), r.center().y), Vec2::new(r.right() + 50.0, r.center().y), &mut build);
    drop(build);
    assert!((hi - 0.9).abs() < 0.05, "{hi}");
    assert_eq!(lo, hi);
}

#[test]
fn knob_turns_by_dragging_and_by_wheel() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let mut v = 0.5f32;
    let mut build = |ui: &mut Ui| {
        ui.knob("k", "Volume", &mut v);
    };
    settle(&mut ui, &mut t, &mut build);
    let c = ui.rect_of(ui.id("k")).unwrap();
    let center = Vec2::new(c.center().x, c.y + 30.0);
    // Up by 80 px: half of the 160 px that span the range.
    drag(&mut ui, &mut t, center, center - Vec2::new(0.0, 80.0), &mut build);
    ui.pointer_moved(center);
    ui.scroll_wheel(-60.0);
    frame(&mut ui, &mut t, &mut build);
    drop(build);
    assert!((v - 0.9).abs() < 0.02, "{v}");
}

#[test]
fn rating_sets_and_clears() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let stars = std::cell::Cell::new(0u8);
    let mut build = |ui: &mut Ui| {
        let mut v = stars.get();
        ui.rating("stars", &mut v, 5);
        stars.set(v);
    };
    settle(&mut ui, &mut t, &mut build);
    let r = ui.rect_of(ui.id("stars")).unwrap();
    let fourth = Vec2::new(r.x + r.w * 0.7, r.center().y);
    let mut tap = |ui: &mut Ui, t: &mut f64, p: Vec2| {
        ui.pointer_moved(p);
        frame(ui, t, &mut build);
        ui.pointer_pressed();
        frame(ui, t, &mut build);
        ui.pointer_released();
        frame(ui, t, &mut build);
    };
    tap(&mut ui, &mut t, fourth);
    let after_first = stars.get();
    tap(&mut ui, &mut t, fourth);
    assert_eq!((after_first, stars.get()), (4, 0));
}

#[test]
fn swatches_and_xy_pad() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let colors = [Color::hex(0xff0000), Color::hex(0x00ff00), Color::hex(0x0000ff)];
    let (mut pick, mut point) = (0usize, Vec2::new(0.5, 0.5));
    let mut build = |ui: &mut Ui| {
        ui.swatches("sw", &colors, &mut pick);
        ui.xy_pad("pad", &mut point, Vec2::new(200.0, 100.0));
    };
    settle(&mut ui, &mut t, &mut build);
    let third = ui.id("sw").with(2);
    click(&mut ui, &mut t, third, &mut build);
    let pad = ui.rect_of(ui.id("pad")).unwrap();
    drag(&mut ui, &mut t, pad.center(), Vec2::new(pad.x + 50.0, pad.y + 75.0), &mut build);
    drop(build);
    assert_eq!(pick, 2);
    assert!((point.x - 0.25).abs() < 0.01 && (point.y - 0.75).abs() < 0.01, "{point:?}");
}

// ----- wrapped text, pickers, data, palette -----------------------------------

#[test]
fn paragraphs_wrap_to_the_width_and_reflow() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let text = "una frase bastante larga que no cabe en una sola línea de un panel estrecho";
    let mut width = 760.0;
    let build = |ui: &mut Ui, w: f32| {
        ui.card_w("c", w, |ui| ui.paragraph(text));
        ui.button("below", "Below");
    };
    settle(&mut ui, &mut t, |ui| build(ui, width));
    let wide = ui.rect_of(ui.id("below")).unwrap().y;
    width = 220.0;
    settle(&mut ui, &mut t, |ui| build(ui, width));
    // Narrower: more lines, so what follows moves down.
    assert!(ui.rect_of(ui.id("below")).unwrap().y > wide + 30.0);
    assert!(texts(&ui).len() > 4);
}

#[test]
fn text_area_takes_newlines_and_moves_between_lines() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let mut notes = String::new();
    let mut build = |ui: &mut Ui| {
        ui.text_area("notes", &mut notes, "Notes", Vec2::new(300.0, 120.0));
    };
    settle(&mut ui, &mut t, &mut build);
    let id = ui.id("notes");
    click(&mut ui, &mut t, id, &mut build);
    ui.text_input("abc");
    frame(&mut ui, &mut t, &mut build);
    ui.key_pressed(Key::Enter);
    frame(&mut ui, &mut t, &mut build);
    ui.text_input("de");
    frame(&mut ui, &mut t, &mut build);
    // Up to the first line, near the same column, and type there.
    ui.key_pressed(Key::Up);
    frame(&mut ui, &mut t, &mut build);
    ui.text_input("X");
    frame(&mut ui, &mut t, &mut build);
    drop(build);
    assert_eq!(notes, "abXc\nde");
}

#[test]
fn select_and_combobox() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let options = ["Madrid", "Málaga", "Bilbao", "Sevilla"];
    let (mut sel, mut city, mut query) = (0usize, None, String::new());
    let mut build = |ui: &mut Ui| {
        ui.select("s", &options, &mut sel);
        ui.combobox("c", &options, &mut city, &mut query, "City", 240.0);
    };
    settle(&mut ui, &mut t, &mut build);
    let s = ui.id("s");
    click(&mut ui, &mut t, s, &mut build);
    settle(&mut ui, &mut t, &mut build);
    click(&mut ui, &mut t, s.child("option").with(2), &mut build);
    settle(&mut ui, &mut t, &mut build);
    // Combobox: "mala" matches "Málaga" (no accents needed); Enter picks it.
    let c = ui.id("c");
    click(&mut ui, &mut t, c, &mut build);
    ui.text_input("mala");
    frame(&mut ui, &mut t, &mut build);
    settle(&mut ui, &mut t, &mut build);
    ui.key_pressed(Key::Enter);
    frame(&mut ui, &mut t, &mut build);
    settle(&mut ui, &mut t, &mut build);
    let focused = ui.has_focus();
    drop(build);
    assert_eq!(sel, 2);
    assert_eq!((city, query.as_str(), focused), (Some(1), "Málaga", false));
}

#[test]
fn table_sorts_and_rows_travel() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let rows: [&[&str]; 3] = [&["Pera", "12 €"], &["Kiwi", "3 €"], &["Uva", "100 €"]];
    let mut sort = None;
    let mut build = |ui: &mut Ui| {
        ui.table("t", &["Fruta", "Precio"], &rows, &mut sort);
    };
    settle(&mut ui, &mut t, &mut build);
    let row_y = |ui: &Ui, i: u64| ui.rect_of(ui.id("t").child("row").with(i)).unwrap().y;
    assert!(row_y(&ui, 0) < row_y(&ui, 1) && row_y(&ui, 1) < row_y(&ui, 2));
    // Sort by price: numeric, not alphabetical (3 < 12 < 100).
    let price = ui.id("t").child("header").with(1);
    click(&mut ui, &mut t, price, &mut build);
    // Mid-way the rows are travelling, not jumping.
    frame(&mut ui, &mut t, &mut build);
    ui.pointer_left();
    settle(&mut ui, &mut t, &mut build);
    assert!(row_y(&ui, 1) < row_y(&ui, 0) && row_y(&ui, 0) < row_y(&ui, 2));
    // Again: descending.
    click(&mut ui, &mut t, price, &mut build);
    ui.pointer_left();
    settle(&mut ui, &mut t, &mut build);
    drop(build);
    assert_eq!(sort, Some((1, true)));
}

#[test]
fn list_reorders_by_dragging() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let mut items = vec!["one".to_string(), "two".to_string(), "three".to_string(), "four".to_string()];
    let mut build = |ui: &mut Ui| {
        ui.reorderable("l", &mut items, |s| s.as_str());
    };
    settle(&mut ui, &mut t, &mut build);
    let first = ui.rect_of(ui.id("l").child("one")).unwrap();
    let third = ui.rect_of(ui.id("l").child("three")).unwrap();
    drag(&mut ui, &mut t, first.center(), Vec2::new(first.center().x, third.center().y + 4.0), &mut build);
    settle(&mut ui, &mut t, &mut build);
    drop(build);
    assert_eq!(items, ["two", "three", "one", "four"]);
}

#[test]
fn command_palette_filters_runs_and_learns() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let commands = [("a", "Abrir archivo"), ("g", "Guardar"), ("e", "Exportar a PDF")];
    let (mut open, mut ran) = (false, Vec::new());
    let run = |ui: &mut Ui, t: &mut f64, open: &mut bool, ran: &mut Vec<usize>, typed: &str| {
        *open = true;
        frame(ui, t, |ui| {
            ui.command_palette("p", open, &commands);
        });
        ui.text_input(typed);
        frame(ui, t, |ui| {
            ui.command_palette("p", open, &commands);
        });
        ui.key_pressed(Key::Enter);
        frame(ui, t, |ui| {
            if let Some(i) = ui.command_palette("p", open, &commands) {
                ran.push(i);
            }
        });
        settle(ui, t, |ui| {
            ui.command_palette("p", open, &commands);
        });
    };
    // Scattered letters match: "exp pdf" runs "Exportar a PDF".
    run(&mut ui, &mut t, &mut open, &mut ran, "exppdf");
    assert_eq!(ran, [2]);
    assert!(!open && !ui.has_focus());
    // Used a few times, "Guardar" becomes what Enter runs with nothing typed.
    for _ in 0..3 {
        run(&mut ui, &mut t, &mut open, &mut ran, "guar");
    }
    run(&mut ui, &mut t, &mut open, &mut ran, "");
    assert_eq!(ran.last(), Some(&1));
}

#[test]
fn text_area_ignores_the_caret_of_another_field() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let (mut name, mut notes) = (String::new(), String::from("árbol"));
    let mut build = |ui: &mut Ui| {
        ui.text_field("name", &mut name, "Name", 200.0);
        ui.text_area("notes", &mut notes, "Notes", Vec2::new(200.0, 80.0));
    };
    settle(&mut ui, &mut t, &mut build);
    let field = ui.id("name");
    click(&mut ui, &mut t, field, &mut build);
    // A caret position that would fall inside "á" in the other field.
    ui.text_input("x");
    frame(&mut ui, &mut t, &mut build);
}
