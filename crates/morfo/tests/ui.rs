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
    // What a dialog holds is the dialog's: its id is under the dialog's.
    let (trigger, below, ok) = (ui.id("trigger"), ui.id("under"), ui.id("dlg").child("ok"));
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
    let in_b = acc.with(1).child("in-b");
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

// ----- selection, clipboard, input methods, keyboard and accessibility ----------

use morfo::{Modifiers, Role};

#[test]
fn text_selection_clipboard_and_paste() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let (mut line, mut notes) = (String::from("hola mundo"), String::new());
    let mut build = |ui: &mut Ui| {
        ui.text_field("line", &mut line, "", 300.0);
        ui.text_area("notes", &mut notes, "", Vec2::new(300.0, 100.0));
    };
    settle(&mut ui, &mut t, &mut build);
    let field = ui.id("line");
    click(&mut ui, &mut t, field, &mut build);
    // End, then Shift+Left ×5 selects "mundo"; copy it.
    ui.key_pressed(Key::End);
    ui.set_modifiers(Modifiers { shift: true, ctrl: false });
    for _ in 0..5 {
        ui.key_pressed(Key::Left);
    }
    frame(&mut ui, &mut t, &mut build);
    ui.set_modifiers(Modifiers::default());
    ui.key_pressed(Key::Copy);
    frame(&mut ui, &mut t, &mut build);
    assert_eq!(ui.take_clipboard().as_deref(), Some("mundo"));
    // Typing replaces the selection.
    ui.text_input("gente");
    frame(&mut ui, &mut t, &mut build);
    // Pasting into the multi-line field keeps the line break.
    let area = ui.id("notes");
    click(&mut ui, &mut t, area, &mut build);
    ui.paste("uno\ndos");
    frame(&mut ui, &mut t, &mut build);
    ui.key_pressed(Key::SelectAll);
    ui.key_pressed(Key::Cut);
    frame(&mut ui, &mut t, &mut build);
    let cut = ui.take_clipboard();
    drop(build);
    assert_eq!(line, "hola gente");
    assert_eq!(cut.as_deref(), Some("uno\ndos"));
    assert_eq!(notes, "");
}

#[test]
fn dragging_selects_text() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let mut text = String::from("abcdefghij");
    let mut build = |ui: &mut Ui| {
        ui.text_field("f", &mut text, "", 300.0);
    };
    settle(&mut ui, &mut t, &mut build);
    let r = ui.rect_of(ui.id("f")).unwrap();
    let x0 = r.x + 12.6; // the field's padding
                         // ApproxText: 8.1 px per character at the default size.
    drag(&mut ui, &mut t, Vec2::new(x0 + 8.1 * 2.0, r.center().y), Vec2::new(x0 + 8.1 * 5.0, r.center().y), &mut build);
    ui.key_pressed(Key::Copy);
    frame(&mut ui, &mut t, &mut build);
    assert_eq!(ui.take_clipboard().as_deref(), Some("cde"));
}

#[test]
fn input_method_composition() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let mut text = String::from("a");
    let mut build = |ui: &mut Ui| {
        ui.text_field("f", &mut text, "", 300.0);
    };
    settle(&mut ui, &mut t, &mut build);
    let field = ui.id("f");
    click(&mut ui, &mut t, field, &mut build);
    // While composing, the text is shown but not committed.
    ui.ime_preedit("´");
    frame(&mut ui, &mut t, &mut build);
    assert!(texts(&ui).contains(&"a´".to_string()));
    assert!(ui.ime_area().is_some());
    ui.ime_preedit("");
    ui.text_input("é");
    frame(&mut ui, &mut t, &mut build);
    drop(build);
    assert_eq!(text, "aé");
}

#[test]
fn every_control_works_from_the_keyboard() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let (mut clicks, mut on, mut level) = (0, false, 0.5f32);
    let mut build = |ui: &mut Ui| {
        if ui.button("b", "Go").clicked {
            clicks += 1;
        }
        ui.toggle("t", &mut on);
        ui.slider("s", &mut level, 200.0);
    };
    settle(&mut ui, &mut t, &mut build);
    // Tab reaches the button; Enter presses it.
    ui.key_pressed(Key::Tab);
    frame(&mut ui, &mut t, &mut build);
    assert_eq!(ui.focused(), Some(ui.id("b")));
    ui.key_pressed(Key::Enter);
    frame(&mut ui, &mut t, &mut build);
    // A focus ring is drawn around it (an unfilled, stroked rectangle).
    let ring =
        ui.draw_list().commands().iter().any(|c| matches!(c, DrawCmd::Rect { fill, stroke_width, .. } if fill.a == 0.0 && *stroke_width == 2.0));
    assert!(ring);
    // Tab to the switch, Space flips it; Tab to the slider, arrows move it.
    ui.key_pressed(Key::Tab);
    frame(&mut ui, &mut t, &mut build);
    ui.key_pressed(Key::Space);
    frame(&mut ui, &mut t, &mut build);
    ui.key_pressed(Key::Tab);
    frame(&mut ui, &mut t, &mut build);
    ui.key_pressed(Key::Right);
    ui.key_pressed(Key::Right);
    frame(&mut ui, &mut t, &mut build);
    // Shift+Tab goes back.
    ui.set_modifiers(Modifiers { shift: true, ctrl: false });
    ui.key_pressed(Key::Tab);
    frame(&mut ui, &mut t, &mut build);
    let back = ui.focused();
    drop(build);
    assert_eq!((clicks, on), (1, true));
    assert!((level - 0.6).abs() < 1e-4, "{level}");
    assert_eq!(back, Some(ui.id("t")));
}

#[test]
fn accessibility_description_and_actions() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let (mut clicks, mut on) = (0, true);
    let mut build = |ui: &mut Ui| {
        ui.heading("Settings");
        if ui.button("save", "Save").clicked {
            clicks += 1;
        }
        ui.checkbox("c", "Notify me", &mut on);
    };
    settle(&mut ui, &mut t, &mut build);
    // Off by default: nothing is described, nothing costs.
    assert!(ui.access_nodes().is_empty());
    ui.set_accessibility(true);
    frame(&mut ui, &mut t, &mut build);
    let nodes = ui.access_nodes().to_vec();
    let find = |role: Role| nodes.iter().find(|n| n.role == role).unwrap().clone();
    assert_eq!(find(Role::Heading).label, "Settings");
    assert_eq!(find(Role::Button).label, "Save");
    assert_eq!(find(Role::CheckBox).checked, Some(true));
    // A screen reader's "click" activates the element.
    let save = ui.id("save");
    ui.access_click(save);
    frame(&mut ui, &mut t, &mut build);
    drop(build);
    assert_eq!(clicks, 1);
}

// ----- richer layout and filled shapes --------------------------------------------

use morfo::Align;

#[test]
fn grid_columns_and_alignment() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let build = |ui: &mut Ui| {
        ui.grid(3, |ui| {
            for i in 0..5 {
                let w = ui.available_width();
                ui.surface(&format!("cell{i}"), Vec2::new(w, 20.0 + i as f32 * 10.0), 4.0, Color::WHITE);
            }
        });
        ui.columns(&[1.0, 3.0], |ui, i| {
            let w = ui.available_width();
            ui.surface(&format!("col{i}"), Vec2::new(w, 30.0), 4.0, Color::WHITE);
        });
        ui.align(Align::End, |ui| {
            ui.button("end", "End");
        });
        ui.align(Align::Center, |ui| {
            ui.button("mid", "Mid");
        });
    };
    settle(&mut ui, &mut t, build);
    let r = |k: &str| ui.rect_of(ui.id(k)).unwrap();
    // Three equal cells per row, rows as tall as their tallest cell.
    assert_eq!(r("cell0").y, r("cell2").y);
    assert!((r("cell0").w - r("cell1").w).abs() < 0.01);
    assert!(r("cell1").x > r("cell0").right() && r("cell2").x > r("cell1").right());
    assert_eq!(r("cell3").x, r("cell0").x);
    assert!(r("cell3").y >= r("cell2").bottom());
    // Weighted columns: the second is three times as wide, both top-aligned.
    assert!((r("col1").w / r("col0").w - 3.0).abs() < 0.01);
    assert_eq!(r("col0").y, r("col1").y);
    assert!(r("col0").y >= r("cell4").bottom());
    // Aligned to the end of the row, and centered.
    let right = VIEW.x - 27.0;
    assert!((r("end").right() - right).abs() < 1.0, "{:?}", r("end"));
    assert!((r("mid").center().x - VIEW.x * 0.5).abs() < 1.0, "{:?}", r("mid"));
}

#[test]
fn filled_polygons_and_arcs() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let (mut stars, mut v) = (3u8, 0.5f32);
    let mut build = |ui: &mut Ui| {
        ui.rating("r", &mut stars, 5);
        ui.knob("k", "Knob", &mut v);
    };
    settle(&mut ui, &mut t, &mut build);
    let list = ui.draw_list();
    let polys: Vec<usize> = list
        .commands()
        .iter()
        .filter_map(|c| if let DrawCmd::Polygon { start, len, .. } = *c { Some(list.points(start, len).len()) } else { None })
        .collect();
    // Three filled stars (10 points each) and the knob's two arcs.
    assert_eq!(polys.iter().filter(|&&n| n == 10).count(), 3);
    assert!(polys.iter().filter(|&&n| n > 20).count() >= 2);
}

// ----- the adaptive layer -----------------------------------------------------------

use morfo::Reason;

#[test]
fn habits_have_names_and_can_be_frozen_or_forgotten() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let mut build = |ui: &mut Ui| {
        ui.button("save", "Guardar");
        ui.button("open", "Abrir");
    };
    settle(&mut ui, &mut t, &mut build);
    let (save, open) = (ui.id("save"), ui.id("open"));
    for _ in 0..3 {
        click(&mut ui, &mut t, save, &mut build);
    }
    click(&mut ui, &mut t, open, &mut build);
    let habits = ui.habits();
    assert_eq!(habits[0].label, "Guardar");
    assert_eq!(habits[0].count, 3);
    assert!(matches!(habits[0].reason, Some(Reason::Frequent(3) | Reason::Recent)));
    // Frozen: using things teaches nothing.
    ui.profile_mut().frozen = true;
    click(&mut ui, &mut t, open, &mut build);
    assert_eq!(ui.profile().count(open), 1);
    ui.profile_mut().frozen = false;
    ui.profile_mut().forget(save);
    assert_eq!(ui.habits().len(), 1);
}

#[test]
fn the_next_step_is_offered() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let items = [("new", "Nuevo"), ("open", "Abrir"), ("save", "Guardar")];
    let build = |ui: &mut Ui| {
        ui.adaptive_actions("bar", &items);
        ui.predicted_action("next", "Next:", "bar", &items);
    };
    settle(&mut ui, &mut t, build);
    let bar = ui.id("bar");
    let offered = |ui: &Ui| ui.rect_of(ui.id("next").child("open")).is_some();
    // "Nuevo" then "Abrir", three times.
    for _ in 0..3 {
        click(&mut ui, &mut t, bar.child("new"), build);
        click(&mut ui, &mut t, bar.child("open"), build);
        ui.pointer_left();
        t += 30.0;
        settle(&mut ui, &mut t, build);
    }
    assert!(!offered(&ui) || ui.profile().predict(t).is_none());
    // After "Nuevo", "Abrir" is offered right away.
    click(&mut ui, &mut t, bar.child("new"), build);
    ui.pointer_left();
    settle(&mut ui, &mut t, build);
    assert!(offered(&ui));
    assert_eq!(ui.why(bar.child("open")), Some(Reason::AfterPrevious(bar.child("new"))));
}

#[test]
fn dashboard_tiles_follow_use() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let tiles = [("mail", "Mail"), ("calendar", "Calendar"), ("notes", "Notes"), ("photos", "Photos")];
    let build = |ui: &mut Ui| {
        ui.adaptive_dashboard("home", &tiles, 3, |ui, i, _| ui.button(&format!("open-{i}"), "Open").clicked);
    };
    settle(&mut ui, &mut t, build);
    let tile = |ui: &Ui, k: &str| ui.rect_of(ui.id("home").child(k)).unwrap();
    let (photos_before, mail_before) = (tile(&ui, "photos"), tile(&ui, "mail"));
    assert!(photos_before.y > mail_before.y, "four tiles in rows of three");
    for _ in 0..5 {
        let b = ui.id("home").child("photos").child("open-3");
        click(&mut ui, &mut t, b, build);
    }
    ui.pointer_left();
    settle(&mut ui, &mut t, build);
    let (photos, mail) = (tile(&ui, "photos"), tile(&ui, "mail"));
    // The used tile moved to the front and grew to two columns.
    assert!(photos.x <= mail.x && photos.y <= mail.y);
    assert!(photos.w > mail.w * 1.8, "{} vs {}", photos.w, mail.w);
}

#[test]
fn density_follows_experience() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let build = |ui: &mut Ui| {
        ui.button("b", "Button");
    };
    ui.set_adaptive_density(true);
    settle(&mut ui, &mut t, build);
    let before = ui.rect_of(ui.id("b")).unwrap();
    assert!((ui.density() - 1.0).abs() < 1e-3);
    // A lot of use, lately.
    for i in 0..200 {
        ui.profile_mut().record(morfo::Id(i % 7), t);
    }
    settle(&mut ui, &mut t, build);
    let after = ui.rect_of(ui.id("b")).unwrap();
    assert!(ui.density() < 0.85, "{}", ui.density());
    assert!(after.h < before.h && after.w < before.w);
}

#[test]
fn anything_can_animate_out() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let build = |ui: &mut Ui, shown: bool| {
        ui.transient("note", shown, |ui| {
            ui.label("Saved");
            ui.label("Your changes are safe");
        });
        ui.button("below", "Below");
    };
    settle(&mut ui, &mut t, |ui| build(ui, true));
    let y0 = ui.rect_of(ui.id("below")).unwrap().y;
    // Hidden: it is still drawn for a while, fading, while the gap closes.
    for _ in 0..4 {
        frame(&mut ui, &mut t, |ui| build(ui, false));
    }
    assert!(texts(&ui).contains(&"Saved".to_string()));
    settle(&mut ui, &mut t, |ui| build(ui, false));
    assert!(!texts(&ui).contains(&"Saved".to_string()));
    assert!(ui.rect_of(ui.id("below")).unwrap().y < y0 - 20.0);
}

// ----- course index, search, flexible space -----------------------------------------

#[test]
fn fill_pushes_the_rest_of_a_row_to_its_end() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let build = |ui: &mut Ui| {
        ui.row(|ui| {
            ui.button("left", "Left");
            ui.fill();
            ui.button("right", "Right");
        });
    };
    settle(&mut ui, &mut t, build);
    let right = ui.rect_of(ui.id("right")).unwrap();
    assert!((right.right() - (VIEW.x - 27.0)).abs() < 1.0, "{right:?}");
    assert!(ui.rect_of(ui.id("left")).unwrap().x < 40.0);
}

#[test]
fn course_index_tracks_progress_and_navigates() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let modules: [(&str, &[&str]); 2] = [("Basics", &["One", "Two"]), ("More", &["Three"])];
    let mut at = 0usize;
    let mut build = |ui: &mut Ui| {
        ui.course_nav("course", "Course", &modules, &mut at);
    };
    settle(&mut ui, &mut t, &mut build);
    let row = |ui: &Ui, i: u64| ui.id("course").with(i);
    // The current lesson counts as visited; the others not yet.
    assert_eq!(ui.profile().count(row(&ui, 0)), 1);
    assert_eq!(ui.profile().count(row(&ui, 2)), 0);
    // Unfold from the edge and pick the third lesson.
    ui.pointer_moved(Vec2::new(3.0, 300.0));
    settle(&mut ui, &mut t, &mut build);
    let third = row(&ui, 2);
    click(&mut ui, &mut t, third, &mut build);
    settle(&mut ui, &mut t, &mut build);
    drop(build);
    assert_eq!(at, 2);
    assert!(ui.profile().count(third) >= 1);
}

#[test]
fn search_unfolds_finds_and_folds_back() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let items = [("a", "Abrir archivo"), ("g", "Guardar"), ("e", "Exportar a PDF")];
    let open = std::cell::Cell::new(false);
    let mut found = None;
    let mut build = |ui: &mut Ui| {
        let mut o = open.get();
        if let Some(i) = ui.search("find", "Search", &mut o, &items, 240.0) {
            found = Some(i);
        }
        open.set(o);
    };
    settle(&mut ui, &mut t, &mut build);
    // Folded: just the magnifier.
    let icon = ui.id("find").child("boton");
    let narrow = ui.rect_of(icon).unwrap().w;
    click(&mut ui, &mut t, icon, &mut build);
    settle(&mut ui, &mut t, &mut build);
    // Unfolded into a focused field.
    assert!(ui.has_focus());
    assert!(ui.rect_of(ui.id("find").child("campo")).unwrap().w > narrow * 3.0);
    ui.text_input("pdf");
    frame(&mut ui, &mut t, &mut build);
    ui.key_pressed(Key::Enter);
    frame(&mut ui, &mut t, &mut build);
    settle(&mut ui, &mut t, &mut build);
    let focus_after = ui.has_focus();
    // Opened from outside (a shortcut), Escape folds it.
    open.set(true);
    frame(&mut ui, &mut t, &mut build);
    frame(&mut ui, &mut t, &mut build);
    let focus_on_shortcut = ui.has_focus();
    ui.key_pressed(Key::Escape);
    frame(&mut ui, &mut t, &mut build);
    settle(&mut ui, &mut t, &mut build);
    drop(build);
    assert_eq!(found, Some(2));
    assert!(!focus_after && focus_on_shortcut);
    assert!(!open.get());
}

// ----- picking up where it was left ---------------------------------------

#[test]
fn a_relaunch_restores_the_scroll_already_at_rest() {
    let build = |ui: &mut Ui| {
        ui.heading("Top");
        ui.scroll("list", 200.0, |ui| {
            for i in 0..60 {
                ui.scope(&format!("row{i}"), |ui| {
                    ui.button("b", &format!("Row {i}"));
                });
            }
        });
    };
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    settle(&mut ui, &mut t, build);
    let row = ui.id("list").child("row8").child("b");
    let unscrolled = ui.rect_of(row).unwrap().y;
    ui.pointer_moved(ui.rect_of(ui.id("list")).unwrap().center());
    ui.scroll_wheel(-300.0);
    settle(&mut ui, &mut t, build);
    let y = ui.rect_of(row).unwrap().y;
    assert!((unscrolled - y - 300.0).abs() < 0.5);
    let saved = ui.save_state();

    // The next run has nothing but that text. A few unpainted frames with
    // no animation, and its first visible frame is the old one.
    let (mut next, mut t) = (Ui::new(Theme::dark()), 0.0);
    assert!(next.restore_state(&saved));
    next.set_instant(true);
    for _ in 0..4 {
        frame(&mut next, &mut t, build);
    }
    next.set_instant(false);
    assert!(!frame(&mut next, &mut t, build), "already at rest");
    assert!((next.rect_of(row).unwrap().y - y).abs() < 0.5);
    assert_eq!(next.save_state(), saved);

    // A page that got shorter is scrolled as far as it still goes.
    let (mut shorter, mut t) = (Ui::new(Theme::dark()), 0.0);
    assert!(shorter.restore_state(&saved));
    let short = |ui: &mut Ui| {
        ui.scroll("list", 200.0, |ui| {
            for i in 0..3 {
                ui.scope(&format!("row{i}"), |ui| {
                    ui.button("b", &format!("Row {i}"));
                });
            }
        });
    };
    settle(&mut shorter, &mut t, short);
    assert!(shorter.rect_of(shorter.id("list").child("row0").child("b")).unwrap().y > 0.0);

    assert!(!next.restore_state("not a saved state"));
    assert!(!next.restore_state("morfo-ui1\ns zz 10\n"));
    assert!(next.restore_state("morfo-ui1\nfuture 1 2 3\n"), "unknown lines are skipped");
}

#[test]
fn a_notice_stays_while_declared_and_then_leaves() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let text = std::cell::Cell::new(Some("Rebuilding…"));
    let build = |ui: &mut Ui| {
        ui.label("App");
        if let Some(text) = text.get() {
            ui.notice("build", text);
        }
    };
    settle(&mut ui, &mut t, build);
    assert!(texts(&ui).contains(&"Rebuilding…".to_string()));
    // Unlike a toast, it is still there an hour later, costing nothing.
    t += 3600.0;
    assert!(!frame(&mut ui, &mut t, build));
    assert!(texts(&ui).contains(&"Rebuilding…".to_string()));
    // Long text wraps instead of leaving the window.
    text.set(Some("error[E0425]: cannot find value `nope` in this scope, and the explanation goes on and on for far longer than the window is wide"));
    settle(&mut ui, &mut t, build);
    assert!(texts(&ui).len() > 2, "wrapped into several lines");
    // No longer declared: it animates out and is gone.
    text.set(None);
    assert!(frame(&mut ui, &mut t, build));
    settle(&mut ui, &mut t, build);
    assert_eq!(texts(&ui), ["App"]);
}

// ----- charts, navigation, forms, status and panes -------------------------

use morfo::{Date, Shortcut, Side};
use std::cell::{Cell, RefCell};

fn press_at(ui: &mut Ui, t: &mut f64, p: Vec2, mut f: impl FnMut(&mut Ui)) {
    ui.pointer_moved(p);
    frame(ui, t, &mut f);
    ui.pointer_pressed();
    frame(ui, t, &mut f);
}

fn release(ui: &mut Ui, t: &mut f64, mut f: impl FnMut(&mut Ui)) {
    ui.pointer_released();
    frame(ui, t, &mut f);
}

#[test]
fn charts_travel_to_new_data_and_report_clicks() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let data = RefCell::new(vec![10.0f32, 40.0, 25.0]);
    let (bar, cell) = (Cell::new(None), Cell::new(None));
    let build = |ui: &mut Ui| {
        let v = data.borrow();
        if let Some(i) = ui.bar_chart("bars", &["a", "b", "c"], &v, Vec2::new(400.0, 160.0)) {
            bar.set(Some(i));
        }
        ui.line_chart("lines", &["1", "2", "3"], &[("x", &v[..]), ("y", &[5.0, -5.0, 5.0])], Vec2::new(400.0, 120.0));
        ui.sparkline(&v, 120.0);
        if let Some(c) = ui.heatmap("heat", &["r1", "r2"], &["c1", "c2", "c3"], &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]) {
            cell.set(Some(c));
        }
        ui.gauge("gauge", "Uso", v[1] / 50.0, 120.0);
    };
    settle(&mut ui, &mut t, build);
    let b = ui.id("bars").child("b");
    let tall = ui.rect_of(b).unwrap().h;
    data.borrow_mut()[1] = 5.0;
    assert!(frame(&mut ui, &mut t, build), "new data is a movement, not a jump");
    assert!(ui.rect_of(b).unwrap().h > tall * 0.5);
    settle(&mut ui, &mut t, build);
    assert!(ui.rect_of(b).unwrap().h < tall * 0.5);
    let c = ui.id("bars").child("c");
    click(&mut ui, &mut t, c, build);
    assert_eq!(bar.get(), Some(2));
    let last = ui.id("heat").with(5);
    click(&mut ui, &mut t, last, build);
    assert_eq!(cell.get(), Some((1, 2)));
    // Hovering a bar shows its value.
    ui.pointer_moved(ui.rect_of(c).unwrap().center());
    settle(&mut ui, &mut t, build);
    assert!(texts(&ui).iter().any(|s| s == "c: 25"));
    ui.pointer_left();
    settle(&mut ui, &mut t, build);
    assert!(!texts(&ui).iter().any(|s| s == "c: 25"), "the card fades away");
}

#[test]
fn tabs_pagination_and_breadcrumbs() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let (tab, page, crumb) = (Cell::new(0usize), Cell::new(0usize), Cell::new(None));
    let build = |ui: &mut Ui| {
        let mut s = tab.get();
        ui.tabs("tabs", &["Uno", "Dos", "Tres"], &mut s);
        tab.set(s);
        let mut p = page.get();
        ui.pagination("pag", 20, &mut p);
        page.set(p);
        if let Some(i) = ui.breadcrumbs("path", &["Inicio", "Documentos", "Informe"]) {
            crumb.set(Some(i));
        }
    };
    settle(&mut ui, &mut t, build);
    let third = ui.id("tabs").with(2);
    click(&mut ui, &mut t, third, build);
    assert_eq!(tab.get(), 2);
    ui.key_pressed(Key::Left);
    frame(&mut ui, &mut t, build);
    assert_eq!(tab.get(), 1, "the arrows move between tabs");

    let last = ui.id("pag").child("page").with(19);
    click(&mut ui, &mut t, last, build);
    assert_eq!(page.get(), 19);
    settle(&mut ui, &mut t, build);
    let back = ui.id("pag").child("arrow").with(0);
    click(&mut ui, &mut t, back, build);
    assert_eq!(page.get(), 18);
    settle(&mut ui, &mut t, build);
    assert!(texts(&ui).iter().any(|s| s == "16"), "the window of pages followed");

    let docs = ui.id("path").child("Documentos").with(1);
    click(&mut ui, &mut t, docs, build);
    assert_eq!(crumb.get(), Some(1));

    // In a narrow space the middle of a long path folds into "…".
    let mut narrow = Ui::new(Theme::dark());
    let long = ["Inicio", "Documentos", "Proyectos", "2026", "Noviembre", "Informe final"];
    for i in 0..4 {
        narrow.begin(i as f64 / 60.0, Vec2::new(380.0, 300.0));
        narrow.breadcrumbs("path", &long);
        narrow.end();
    }
    let shown = texts(&narrow);
    assert!(shown.contains(&"…".to_string()) && shown.contains(&"Informe final".to_string()));
    assert!(!shown.contains(&"Proyectos".to_string()));
}

#[test]
fn a_tree_opens_and_closes_its_branches() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let items = [(0, "src"), (1, "lib.rs"), (1, "ui"), (2, "mod.rs"), (0, "README.md")];
    let sel = Cell::new(None);
    let build = |ui: &mut Ui| {
        let mut s = sel.get();
        ui.tree("tree", &items, &mut s);
        sel.set(s);
    };
    settle(&mut ui, &mut t, build);
    assert!(!texts(&ui).contains(&"lib.rs".to_string()), "branches start closed");
    let src = ui.id("tree").child("src");
    click(&mut ui, &mut t, src, build);
    settle(&mut ui, &mut t, build);
    assert_eq!(sel.get(), Some(0));
    assert!(texts(&ui).contains(&"lib.rs".to_string()));
    let lib = src.child("lib.rs");
    click(&mut ui, &mut t, lib, build);
    assert_eq!(sel.get(), Some(1));
    click(&mut ui, &mut t, src, build);
    settle(&mut ui, &mut t, build);
    assert!(!texts(&ui).contains(&"lib.rs".to_string()), "closed again");
    assert!(texts(&ui).contains(&"README.md".to_string()));
}

#[test]
fn a_drawer_slides_in_and_closes_outside() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let open = Cell::new(true);
    let build = |ui: &mut Ui| {
        let mut o = open.get();
        ui.label("Debajo");
        ui.drawer("panel", "Ajustes", Side::Right, &mut o, 300.0, |ui| ui.label("Dentro"));
        open.set(o);
    };
    settle(&mut ui, &mut t, build);
    assert!(texts(&ui).contains(&"Dentro".to_string()));
    press_at(&mut ui, &mut t, Vec2::new(100.0, 300.0), build);
    release(&mut ui, &mut t, build);
    assert!(!open.get(), "a click outside closes it");
    settle(&mut ui, &mut t, build);
    assert!(!texts(&ui).contains(&"Dentro".to_string()));
    open.set(true);
    settle(&mut ui, &mut t, build);
    ui.key_pressed(Key::Escape);
    frame(&mut ui, &mut t, build);
    assert!(!open.get(), "and so does Escape");
}

#[test]
fn date_and_color_pickers() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let date = Cell::new(Date::new(2026, 11, 16));
    let color = Cell::new(Color::hex(0xff0000));
    let build = |ui: &mut Ui| {
        let mut d = date.get();
        ui.date_picker("fecha", &mut d);
        date.set(d);
        let mut c = color.get();
        ui.color_picker("color", &mut c, 240.0);
        color.set(c);
    };
    settle(&mut ui, &mut t, build);
    let field = ui.id("fecha");
    click(&mut ui, &mut t, field, build);
    settle(&mut ui, &mut t, build);
    let day = field.child("day").with(20);
    click(&mut ui, &mut t, day, build);
    assert_eq!(date.get(), Date::new(2026, 11, 20));
    settle(&mut ui, &mut t, build);
    // Next month, then its first day.
    click(&mut ui, &mut t, field, build);
    settle(&mut ui, &mut t, build);
    let next = field.child("siguiente");
    click(&mut ui, &mut t, next, build);
    settle(&mut ui, &mut t, build);
    let first = field.child("day").with(1);
    click(&mut ui, &mut t, first, build);
    assert_eq!(date.get(), Date::new(2026, 12, 1));
    settle(&mut ui, &mut t, build);
    // With the field focused, the arrows move by days and weeks.
    ui.key_pressed(Key::Right);
    frame(&mut ui, &mut t, build);
    ui.key_pressed(Key::Down);
    frame(&mut ui, &mut t, build);
    assert_eq!(date.get(), Date::new(2026, 12, 9));

    // The middle of the hue bar is cyan; the bottom of the square, black.
    let bar = ui.rect_of(ui.id("color").child("hue")).unwrap();
    press_at(&mut ui, &mut t, bar.center(), build);
    release(&mut ui, &mut t, build);
    assert_eq!(color.get().to_hex(), 0x00ffff);
    let square = ui.rect_of(ui.id("color").child("sv")).unwrap();
    // Dragged past the bottom-left corner it clamps to black.
    press_at(&mut ui, &mut t, Vec2::new(square.x + 1.0, square.bottom() - 1.0), build);
    ui.pointer_moved(Vec2::new(square.x - 50.0, square.bottom() + 50.0));
    frame(&mut ui, &mut t, build);
    release(&mut ui, &mut t, build);
    assert_eq!(color.get().to_hex(), 0x000000);
    // Back to full brightness, it is still cyan: the picker kept the hue,
    // which black alone does not have.
    press_at(&mut ui, &mut t, Vec2::new(square.right() - 1.0, square.y + 1.0), build);
    release(&mut ui, &mut t, build);
    let hsv = color.get().to_hsv();
    assert!((hsv.h - 0.5).abs() < 0.01 && hsv.s > 0.98 && hsv.v > 0.98, "{hsv:?}");
}

#[test]
fn tags_multiple_choice_and_shortcuts() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let tags = RefCell::new(Vec::<String>::new());
    let picked = RefCell::new(Vec::<usize>::new());
    let shortcut = RefCell::new(None::<Shortcut>);
    let build = |ui: &mut Ui| {
        ui.tag_input("tags", &mut tags.borrow_mut(), "Añade etiquetas", 360.0);
        ui.multi_select("multi", &["Alpha", "Beta", "Gamma"], &mut picked.borrow_mut(), "Elige", 300.0);
        ui.shortcut_input("atajo", &mut shortcut.borrow_mut(), 240.0);
    };
    settle(&mut ui, &mut t, build);
    let tag_box = ui.id("tags");
    click(&mut ui, &mut t, tag_box, build);
    assert!(ui.has_focus(), "a click on the box goes to its field");
    ui.text_input("rust");
    frame(&mut ui, &mut t, build);
    ui.key_pressed(Key::Enter);
    frame(&mut ui, &mut t, build);
    ui.text_input("ui, web,");
    frame(&mut ui, &mut t, build);
    assert_eq!(*tags.borrow(), ["rust", "ui", "web"]);
    ui.key_pressed(Key::Backspace);
    frame(&mut ui, &mut t, build);
    assert_eq!(*tags.borrow(), ["rust", "ui"], "Backspace on an empty field removes the last one");
    settle(&mut ui, &mut t, build);
    let x = tag_box.child("tag").child("rust").child("x");
    click(&mut ui, &mut t, x, build);
    assert_eq!(*tags.borrow(), ["ui"]);

    let multi = ui.id("multi");
    click(&mut ui, &mut t, multi, build);
    settle(&mut ui, &mut t, build);
    assert!(ui.has_focus(), "the search box has the focus");
    ui.text_input("gam");
    frame(&mut ui, &mut t, build);
    ui.key_pressed(Key::Enter);
    frame(&mut ui, &mut t, build);
    assert_eq!(*picked.borrow(), [2]);
    // Choosing does not close the list: a click on another option adds it.
    settle(&mut ui, &mut t, build);
    ui.key_pressed(Key::Backspace);
    ui.key_pressed(Key::Backspace);
    ui.key_pressed(Key::Backspace);
    frame(&mut ui, &mut t, build);
    settle(&mut ui, &mut t, build);
    let alpha = multi.child("op").with(0);
    click(&mut ui, &mut t, alpha, build);
    assert_eq!(*picked.borrow(), [2, 0]);
    ui.key_pressed(Key::Escape);
    frame(&mut ui, &mut t, build);
    settle(&mut ui, &mut t, build);
    assert_eq!(*picked.borrow(), [2, 0], "they stay chosen after closing");
    assert!(texts(&ui).contains(&"Gamma".to_string()) && texts(&ui).contains(&"Alpha".to_string()));

    let field = ui.id("atajo");
    click(&mut ui, &mut t, field, build);
    assert!(ui.wants_keys());
    ui.set_modifiers(morfo::Modifiers { shift: true, ctrl: true });
    ui.chord("k");
    frame(&mut ui, &mut t, build);
    ui.set_modifiers(morfo::Modifiers::default());
    assert_eq!(shortcut.borrow().as_ref().map(|s| s.to_string()), Some("Ctrl+Shift+K".to_string()));
    assert!(!ui.wants_keys(), "recorded, it lets go of the keyboard");
    settle(&mut ui, &mut t, build);
    assert!(texts(&ui).contains(&"Shift".to_string()));
    click(&mut ui, &mut t, field, build);
    ui.key_pressed(Key::Backspace);
    frame(&mut ui, &mut t, build);
    assert!(shortcut.borrow().is_none());
}

#[test]
fn skeleton_badges_and_steps() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let (loading, count, step) = (Cell::new(true), Cell::new(3u32), Cell::new(2usize));
    let build = |ui: &mut Ui| {
        ui.skeleton("card", loading.get(), 3, |ui| ui.label("Cargado"));
        ui.badge("count", count.get());
        ui.icon_button("bell", morfo::Icon::Star, morfo::Variant::Ghost, morfo::Size::Md);
        ui.badge_on("bell", 5);
        let mut s = step.get();
        ui.steps("steps", &["Datos", "Pago", "Fin"], &mut s);
        step.set(s);
    };
    for _ in 0..200 {
        frame(&mut ui, &mut t, build);
    }
    // The shimmer keeps going, at 30 frames a second rather than every one.
    t += 1.0 / 60.0;
    ui.begin(t, VIEW);
    build(&mut ui);
    assert!(ui.end().wake_in.is_some_and(|w| w <= 1.0 / 30.0 + 1e-3), "the shimmer keeps moving while loading");
    assert!(!texts(&ui).contains(&"Cargado".to_string()));
    assert!(texts(&ui).contains(&"3".to_string()) && texts(&ui).contains(&"5".to_string()));
    loading.set(false);
    count.set(120);
    settle(&mut ui, &mut t, build);
    assert!(texts(&ui).contains(&"Cargado".to_string()));
    assert!(texts(&ui).contains(&"99+".to_string()));
    count.set(0);
    settle(&mut ui, &mut t, build);
    assert!(!texts(&ui).contains(&"99+".to_string()), "at zero it goes away");

    // A step ahead cannot be clicked; one done can.
    let ahead = ui.id("steps").with(2);
    step.set(1);
    settle(&mut ui, &mut t, build);
    click(&mut ui, &mut t, ahead, build);
    assert_eq!(step.get(), 1);
    let first = ui.id("steps").with(0);
    click(&mut ui, &mut t, first, build);
    assert_eq!(step.get(), 0);
}

#[test]
fn a_splitter_drags_and_a_board_moves_cards() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let ratio = Cell::new(0.5f32);
    let board = RefCell::new(vec![vec!["a".to_string(), "b".to_string()], vec!["c".to_string()]]);
    let build = |ui: &mut Ui| {
        let mut r = ratio.get();
        ui.splitter("split", &mut r, 160.0, |ui, i| ui.label(if i == 0 { "Izquierda" } else { "Derecha" }));
        ratio.set(r);
        ui.kanban("board", &["Por hacer", "Hecho"], &mut board.borrow_mut());
    };
    settle(&mut ui, &mut t, build);
    let handle = ui.rect_of(ui.id("split").child("divisor")).unwrap();
    press_at(&mut ui, &mut t, handle.center(), build);
    ui.pointer_moved(handle.center() - Vec2::new(150.0, 0.0));
    frame(&mut ui, &mut t, build);
    assert!(ratio.get() < 0.35, "{}", ratio.get());
    ui.pointer_moved(Vec2::new(-500.0, handle.center().y));
    frame(&mut ui, &mut t, build);
    assert_eq!(ratio.get(), 0.15, "panes keep a minimum");
    release(&mut ui, &mut t, build);
    settle(&mut ui, &mut t, build);

    // Drag "a" under "c", in the other column.
    let a = ui.rect_of(ui.id("board").child("a")).unwrap();
    let c = ui.rect_of(ui.id("board").child("c")).unwrap();
    press_at(&mut ui, &mut t, a.center(), build);
    for k in 1..=10 {
        let to = a.center() + (Vec2::new(c.center().x, c.bottom() + c.h * 0.6) - a.center()) * (k as f32 / 10.0);
        ui.pointer_moved(to);
        frame(&mut ui, &mut t, build);
    }
    release(&mut ui, &mut t, build);
    assert_eq!(*board.borrow(), [vec!["b".to_string()], vec!["c".to_string(), "a".to_string()]]);
    settle(&mut ui, &mut t, build);
    let landed = ui.rect_of(ui.id("board").child("a")).unwrap();
    assert!(landed.y > c.y && (landed.x - c.x).abs() < 1.0, "it settled under «c»");
}

// ----- scroll and transitions -------------------------------------------------

use morfo::{Reveal, Transition};

/// Position and opacity of the text `s`, if it is drawn.
fn text_at(ui: &Ui, s: &str) -> Option<(Vec2, f32)> {
    let list = ui.draw_list();
    list.commands().iter().find_map(|c| match *c {
        DrawCmd::Text { pos, color, start, len, .. } if list.text(start, len) == s => Some((pos, color.a)),
        _ => None,
    })
}

/// Rectangles filled with `color`.
fn rects_of(ui: &Ui, color: Color) -> Vec<morfo::Rect> {
    let close = |a: Color| (a.r - color.r).abs() + (a.g - color.g).abs() + (a.b - color.b).abs() < 0.02 && a.a > 0.5;
    ui.draw_list()
        .commands()
        .iter()
        .filter_map(|c| if let DrawCmd::Rect { rect, fill, .. } = *c { close(fill).then_some(rect) } else { None })
        .collect()
}

#[test]
fn content_enters_as_it_scrolls_into_view() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let build = |ui: &mut Ui| {
        ui.scroll("lista", 200.0, |ui| {
            for i in 0..12 {
                ui.scroll_reveal(&format!("r{i}"), Reveal::Rise, |ui| {
                    ui.label(&format!("item {i}"));
                    ui.space(50.0);
                });
            }
        });
    };
    // What is in view enters one after another.
    let mut staggered = false;
    for _ in 0..30 {
        frame(&mut ui, &mut t, build);
        let a0 = text_at(&ui, "item 0").map_or(0.0, |x| x.1);
        let a1 = text_at(&ui, "item 1").map_or(0.0, |x| x.1);
        staggered |= a0 > 0.3 && a1 < a0 - 0.1;
    }
    assert!(staggered, "the second item starts after the first");
    settle(&mut ui, &mut t, build);
    assert!(text_at(&ui, "item 0").unwrap().1 > 0.99);
    assert!(text_at(&ui, "item 7").is_none_or(|x| x.1 < 0.01), "below the view, still waiting");

    // Scrolled into view, it rises into its place.
    let area = ui.rect_of(ui.id("lista")).unwrap();
    ui.pointer_moved(area.center());
    ui.scroll_wheel(-450.0);
    let mut first_y = None;
    for _ in 0..200 {
        let moving = frame(&mut ui, &mut t, build);
        if let Some((p, a)) = text_at(&ui, "item 7") {
            if a > 0.05 && first_y.is_none() {
                first_y = Some(p.y);
            }
        }
        if !moving {
            break;
        }
    }
    let (end, a) = text_at(&ui, "item 7").unwrap();
    assert!(a > 0.99);
    assert!(first_y.unwrap() > end.y + 4.0, "it came from below: {first_y:?} → {}", end.y);
}

#[test]
fn numbers_count_up_when_seen() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let build = |ui: &mut Ui| ui.count_up("descargas", 12480.0, 0, 32.0);
    let mut partial = false;
    for _ in 0..40 {
        frame(&mut ui, &mut t, build);
        partial |= texts(&ui).iter().any(|s| s != "12,480" && !s.is_empty());
    }
    assert!(partial, "it showed the numbers on the way");
    settle(&mut ui, &mut t, build);
    assert_eq!(texts(&ui), ["12,480"]);
}

#[test]
fn a_header_stays_pinned_until_the_next_section_pushes_it() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let build = |ui: &mut Ui| {
        ui.scroll("contactos", 200.0, |ui| {
            for letter in ["A", "B", "C"] {
                ui.sticky_section(
                    letter,
                    |ui| ui.title(letter),
                    |ui| {
                        for i in 0..8 {
                            ui.label(&format!("{letter}{i}"));
                        }
                    },
                );
            }
        });
    };
    settle(&mut ui, &mut t, build);
    let area = ui.rect_of(ui.id("contactos")).unwrap();
    let top = text_at(&ui, "A").unwrap().0.y;
    assert!((top - area.y).abs() < 1.0);

    ui.pointer_moved(area.center());
    ui.scroll_wheel(-100.0);
    settle(&mut ui, &mut t, build);
    assert!((text_at(&ui, "A").unwrap().0.y - area.y).abs() < 1.0, "pinned at the top");
    let order = texts(&ui);
    let (head, row) = (order.iter().position(|s| s == "A").unwrap(), order.iter().position(|s| s == "A3").unwrap());
    assert!(head > row, "drawn over the rows that pass under it");

    // Travelling to «B» brings its header to the top and pushes «A» out.
    ui.scroll_to("B");
    settle(&mut ui, &mut t, build);
    assert!((text_at(&ui, "B").unwrap().0.y - area.y).abs() < 1.5, "B at the top");
    assert!(text_at(&ui, "A").is_none_or(|(p, _)| p.y + 16.0 <= area.y + 1.0), "A pushed away");
}

#[test]
fn a_scroll_area_stretches_past_its_end_and_springs_back() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let progress = Cell::new(-1.0f32);
    let build = |ui: &mut Ui| {
        ui.scroll("lista", 150.0, |ui| {
            progress.set(ui.scroll_progress());
            for i in 0..30 {
                ui.label(&format!("fila {i}"));
            }
        });
    };
    settle(&mut ui, &mut t, build);
    assert_eq!(progress.get(), 0.0);
    let y0 = text_at(&ui, "fila 0").unwrap().0.y;
    ui.pointer_moved(ui.rect_of(ui.id("lista")).unwrap().center());
    ui.scroll_wheel(200.0);
    let mut furthest = y0;
    for _ in 0..200 {
        let moving = frame(&mut ui, &mut t, build);
        furthest = furthest.max(text_at(&ui, "fila 0").unwrap().0.y);
        if !moving {
            break;
        }
    }
    assert!(furthest > y0 + 5.0, "it stretched: {y0} → {furthest}");
    assert_eq!(text_at(&ui, "fila 0").unwrap().0.y, y0, "and came back exactly");
    ui.scroll_wheel(-10_000.0);
    settle(&mut ui, &mut t, build);
    assert_eq!(progress.get(), 1.0);
}

#[test]
fn scrolled_content_moves_as_one_piece() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let build = |ui: &mut Ui| {
        ui.scroll("lista", 200.0, |ui| {
            for i in 0..40 {
                ui.button(&format!("b{i}"), &format!("Fila {i}"));
            }
        });
    };
    settle(&mut ui, &mut t, build);
    let gap = |ui: &Ui| ui.rect_of(ui.find("b3").unwrap()).unwrap().y - ui.rect_of(ui.find("b2").unwrap()).unwrap().y;
    let rest = gap(&ui);
    ui.pointer_moved(ui.rect_of(ui.id("lista")).unwrap().center());
    ui.scroll_wheel(-300.0);
    let start = ui.rect_of(ui.find("b2").unwrap()).unwrap().y;
    for _ in 0..60 {
        frame(&mut ui, &mut t, build);
        assert!((gap(&ui) - rest).abs() < 0.01, "rows keep their spacing while scrolling");
    }
    assert!(ui.rect_of(ui.find("b2").unwrap()).unwrap().y < start - 250.0);
}

#[test]
fn parallax_layers_hold_back_as_the_page_scrolls() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let blue = Color::hex(0x2050e0);
    let build = |ui: &mut Ui| {
        ui.scroll("pagina", 300.0, |ui| {
            ui.space(200.0);
            ui.parallax("paisaje", 120.0, &[0.5], |ui, _| {
                ui.surface("capa", Vec2::new(200.0, 220.0), 0.0, blue);
            });
            ui.space(600.0);
        });
    };
    settle(&mut ui, &mut t, build);
    let window = |ui: &Ui| ui.rect_of(ui.find("paisaje").unwrap()).unwrap();
    let layer = |ui: &Ui| rects_of(ui, blue)[0];
    let (w0, l0) = (window(&ui), layer(&ui));
    ui.pointer_moved(ui.rect_of(ui.id("pagina")).unwrap().center());
    ui.scroll_wheel(-100.0);
    settle(&mut ui, &mut t, build);
    let (w1, l1) = (window(&ui), layer(&ui));
    let page = w0.y - w1.y;
    assert!(page > 50.0);
    assert!(((l0.y - l1.y) - page * 0.5).abs() < 1.0, "the layer moved half as much: {} of {page}", l0.y - l1.y);
}

#[test]
fn a_transition_moves_the_old_view_out_and_the_new_one_in() {
    for style in Transition::ALL {
        let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
        let view = Cell::new(0usize);
        let build = |ui: &mut Ui| {
            let v = view.get();
            ui.transition("vistas", v, style, |ui| ui.label(if v == 0 { "Uno" } else { "Dos" }));
            ui.button("debajo", "Debajo");
        };
        settle(&mut ui, &mut t, build);
        let home = text_at(&ui, "Uno").unwrap().0;
        view.set(1);
        let mut both = false;
        for _ in 0..12 {
            frame(&mut ui, &mut t, build);
            both |= text_at(&ui, "Uno").is_some() && text_at(&ui, "Dos").is_some();
        }
        if style != Transition::Flip {
            assert!(both, "{style:?}: both views are drawn on the way");
        }
        if matches!(style, Transition::Slide | Transition::Shift | Transition::Push) {
            let (p, _) = text_at(&ui, "Dos").unwrap();
            assert!(p.x > home.x + 1.0, "{style:?}: forward comes from the right");
        }
        settle(&mut ui, &mut t, build);
        assert_eq!(texts(&ui), ["Dos", "Debajo"], "{style:?}");
        assert_eq!(text_at(&ui, "Dos").unwrap().0, home, "{style:?}: it lands where the old one was");
        if matches!(style, Transition::Slide | Transition::Shift) {
            view.set(0);
            let mut from = None;
            for _ in 0..30 {
                frame(&mut ui, &mut t, build);
                from = from.or(text_at(&ui, "Uno").map(|x| x.0.x));
            }
            assert!(from.unwrap() < home.x - 1.0, "{style:?}: back comes from the left");
        }
    }
}

#[test]
fn a_shared_element_flies_between_views() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let red = Color::hex(0xe03030);
    let view = Cell::new(0usize);
    let build = |ui: &mut Ui| {
        let v = view.get();
        ui.transition("galeria", v, Transition::Fade, |ui| {
            if v == 0 {
                ui.label("Galería");
                ui.hero("foto", |ui| {
                    ui.surface("mini", Vec2::new(80.0, 60.0), 8.0, red);
                });
            } else {
                ui.space(160.0);
                ui.hero("foto", |ui| {
                    ui.surface("grande", Vec2::new(400.0, 240.0), 8.0, red);
                });
            }
        });
    };
    settle(&mut ui, &mut t, build);
    let small = rects_of(&ui, red)[0];
    view.set(1);
    frame(&mut ui, &mut t, build);
    frame(&mut ui, &mut t, build);
    let flying = rects_of(&ui, red);
    assert_eq!(flying.len(), 1, "the old copy is left out of the view that leaves");
    assert!(flying[0].w < 200.0 && (flying[0].y - small.y).abs() < 80.0, "it starts from the small one: {:?}", flying[0]);
    let mut grew = flying[0].w;
    for _ in 0..200 {
        let moving = frame(&mut ui, &mut t, build);
        let now = rects_of(&ui, red);
        assert_eq!(now.len(), 1);
        assert!(now[0].w >= grew - 1.0, "it keeps growing");
        grew = now[0].w;
        if !moving {
            break;
        }
    }
    let big = rects_of(&ui, red)[0];
    assert!((big.w - 400.0).abs() < 0.5 && big.y > small.y + 100.0, "it lands in the detail view: {big:?}");
}

// ----- components inside components -----------------------------------------

#[test]
fn repeated_components_keep_their_insides_apart() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let clicks = RefCell::new([0, 0]);
    let build = |ui: &mut Ui| {
        for i in 0..2 {
            ui.card(&format!("tarjeta-{i}"), |ui| {
                ui.label(&format!("Tarjeta {i}"));
                if ui.button("borrar", "Borrar").clicked {
                    clicks.borrow_mut()[i] += 1;
                }
            });
        }
    };
    settle(&mut ui, &mut t, build);
    let both = ui.find_all("borrar");
    assert_eq!(both.len(), 2, "two buttons, two elements");
    assert_eq!(both[1], ui.id("tarjeta-1").child("borrar"), "a card's insides are under the card");
    click(&mut ui, &mut t, both[1], build);
    assert_eq!(*clicks.borrow(), [0, 1], "only the button clicked reacts");
    assert_eq!(ui.find("borrar"), Some(both[0]));
    assert_eq!(ui.find("nada"), None);
}

#[test]
fn a_pressable_is_one_button_that_lets_its_controls_work() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let (whole, inner) = (Cell::new(0), Cell::new(0));
    let build = |ui: &mut Ui| {
        if ui
            .pressable("fila", "Abrir la fila", |ui| {
                ui.label("Una fila entera");
                ui.muted("con dos líneas");
                if ui.button("dentro", "Botón").clicked {
                    inner.set(inner.get() + 1);
                }
            })
            .clicked
        {
            whole.set(whole.get() + 1);
        }
    };
    settle(&mut ui, &mut t, build);
    let row = ui.find("fila").unwrap();
    let r = ui.rect_of(row).unwrap();
    // On the text: the whole thing is pressed.
    press_at(&mut ui, &mut t, Vec2::new(r.x + 10.0, r.y + 8.0), build);
    release(&mut ui, &mut t, build);
    assert_eq!((whole.get(), inner.get()), (1, 0));
    // On the button inside: only the button.
    let target = ui.find("dentro").unwrap();
    click(&mut ui, &mut t, target, build);
    assert_eq!((whole.get(), inner.get()), (1, 1));
    // From the keyboard, like any button: Tab to it, then Enter.
    ui.pointer_left();
    ui.key_pressed(Key::Tab);
    frame(&mut ui, &mut t, build);
    assert_eq!(ui.focused(), Some(row));
    ui.key_pressed(Key::Enter);
    frame(&mut ui, &mut t, build);
    assert_eq!(whole.get(), 2);
}

#[test]
fn a_disabled_part_dims_and_ignores_everything() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let on = Cell::new(false);
    let clicks = Cell::new(0);
    let build = |ui: &mut Ui| {
        ui.button("antes", "Antes");
        ui.enabled(on.get(), |ui| {
            ui.card("bloque", |ui| {
                if ui.button("guardar", "Guardar").clicked {
                    clicks.set(clicks.get() + 1);
                }
            });
        });
        ui.button("despues", "Después");
    };
    ui.set_accessibility(true);
    settle(&mut ui, &mut t, build);
    let save = ui.find("guardar").unwrap();
    click(&mut ui, &mut t, save, build);
    assert_eq!(clicks.get(), 0, "a click does nothing");
    assert!(text_at(&ui, "Guardar").unwrap().1 < 0.5, "it is dimmed");
    assert!(ui.access_nodes().iter().any(|n| n.id == save && n.disabled), "and announced as disabled");
    // Tab goes from the button before it to the one after.
    ui.pointer_left();
    let target = ui.find("antes").unwrap();
    click(&mut ui, &mut t, target, build);
    ui.key_pressed(Key::Tab);
    frame(&mut ui, &mut t, build);
    assert_eq!(ui.focused(), ui.find("despues"));
    // Back on: it fades in and works.
    on.set(true);
    settle(&mut ui, &mut t, build);
    assert!(text_at(&ui, "Guardar").unwrap().1 > 0.99);
    click(&mut ui, &mut t, save, build);
    assert_eq!(clicks.get(), 1);
}

#[test]
fn tabs_menus_tables_and_accordions_take_content() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let (tab, open, sort) = (Cell::new(0usize), Cell::new(None), Cell::new(None));
    let (picked, row, done) = (Cell::new(None), Cell::new(None), Cell::new(0));
    let rows: [&[&str]; 2] = [&["Ana", "3"], &["Bruno", "1"]];
    let build = |ui: &mut Ui| {
        let mut sel = tab.get();
        ui.tabs_with("pestanas", &["Bandeja", "Enviados"], &mut sel, |ui, i, on| {
            ui.label(if i == 0 { "Bandeja" } else { "Enviados" });
            if i == 0 {
                ui.badge("sin-leer", 4);
            }
            if on {
                ui.muted("•");
            }
        });
        tab.set(sel);
        if let Some(i) = ui.menu_with("acciones", "Acciones", &[("copiar", "Copiar"), ("pegar", "Pegar")], |ui, i| {
            ui.label(["Copiar", "Pegar"][i]);
            ui.muted(["Ctrl+C", "Ctrl+V"][i]);
        }) {
            picked.set(Some(i));
        }
        let mut s = sort.get();
        if let Some(r) = ui.table_with("personas", &["Nombre", "Tareas"], &rows, &mut s, |ui, r, c| {
            if c == 0 {
                ui.label(rows[r][0]);
            } else if ui.button_with("hecho", "Hecho", morfo::Variant::Soft, morfo::Size::Sm).clicked {
                done.set(done.get() + 1);
            }
        }) {
            row.set(Some(r));
        }
        sort.set(s);
        let mut o = open.get();
        ui.accordion_with(
            "secciones",
            &["Uno", "Dos"],
            &mut o,
            |ui, i, _| {
                ui.label(["Uno", "Dos"][i]);
                ui.muted("con subtítulo");
            },
            |ui, i| ui.label(&format!("Cuerpo {i}")),
        );
        open.set(o);
    };
    settle(&mut ui, &mut t, build);
    let shown = texts(&ui);
    for s in ["Bandeja", "4", "Enviados", "Ana", "Bruno", "Uno", "con subtítulo"] {
        assert!(shown.iter().any(|x| x == s), "«{s}» is drawn: {shown:?}");
    }
    // Tabs: a click on the second selects it.
    let target = ui.id("pestanas").with(1);
    click(&mut ui, &mut t, target, build);
    assert_eq!(tab.get(), 1);
    // Table: the button in a cell takes the click, the row does not.
    let buttons = ui.find_all("hecho");
    assert_eq!(buttons.len(), 2);
    click(&mut ui, &mut t, buttons[1], build);
    assert_eq!((done.get(), row.get()), (1, None));
    // A click elsewhere on the row is the row's.
    let r = ui.rect_of(ui.id("personas").child("row").with(0)).unwrap();
    press_at(&mut ui, &mut t, Vec2::new(r.x + 8.0, r.center().y), build);
    release(&mut ui, &mut t, build);
    assert_eq!(row.get(), Some(0));
    // Accordion: the custom header opens its section.
    let target = ui.id("secciones").with(1);
    click(&mut ui, &mut t, target, build);
    settle(&mut ui, &mut t, build);
    assert!(texts(&ui).iter().any(|x| x == "Cuerpo 1"));
    // Menu: options with two parts, one chosen.
    let target = ui.id("acciones");
    click(&mut ui, &mut t, target, build);
    settle(&mut ui, &mut t, build);
    assert!(texts(&ui).iter().any(|x| x == "Ctrl+V"));
    let target = ui.id("acciones").child("pegar");
    click(&mut ui, &mut t, target, build);
    assert_eq!(picked.get(), Some(1));
}

#[test]
fn a_style_is_inherited_by_what_is_inside() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let green = Color::hex(0x22c3a6);
    let build = |ui: &mut Ui| {
        ui.button("normal", "Normal");
        ui.with_accent(green, |ui| {
            ui.button("verde", "Verde");
        });
        ui.with_size(morfo::Size::Lg, |ui| {
            ui.button("grande", "Grande");
        });
        ui.with_variant(morfo::Variant::Ghost, |ui| {
            ui.button("discreto", "Discreto");
        });
    };
    ui.pointer_left();
    settle(&mut ui, &mut t, build);
    let h = |ui: &Ui, k: &str| ui.rect_of(ui.find(k).unwrap()).unwrap().h;
    assert!(h(&ui, "grande") > h(&ui, "normal") * 1.1, "larger inside with_size(Lg)");
    assert_eq!(rects_of(&ui, green).len(), 1, "only the button inside with_accent is green");
    // A ghost button has no fill at rest: one solid fill fewer.
    let accent = ui.palette()[morfo::theme::ACCENT];
    assert_eq!(rects_of(&ui, accent).len(), 2, "«Normal» and «Grande» are solid; «Discreto» is not");
}

// ----- doing less ------------------------------------------------------------

#[test]
fn a_virtual_list_only_builds_the_rows_in_view() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let built = Cell::new(0);
    let build = |ui: &mut Ui| {
        built.set(0);
        ui.virtual_list("filas", 20_000, 30.0, 300.0, |ui, i| {
            built.set(built.get() + 1);
            ui.label(&format!("Fila {i}"));
        });
    };
    settle(&mut ui, &mut t, build);
    assert!(built.get() <= 11, "{} rows built for a view of ten", built.get());
    assert!(ui.node_count() < 40, "{} elements alive", ui.node_count());
    let list = ui.rect_of(ui.id("filas")).unwrap();
    assert_eq!(text_at(&ui, "Fila 0").unwrap().0.y.round(), (list.y + (30.0 - 15.0 * 1.3) * 0.5).round(), "centered in its row");
    assert!(text_at(&ui, "Fila 15").is_none());

    // Far down the list: the rows there, as they are (no entrance), and
    // the first ones gone.
    ui.pointer_moved(list.center());
    ui.scroll_wheel(-30.0 * 5000.0);
    for _ in 0..200 {
        if !frame(&mut ui, &mut t, build) {
            break;
        }
        for (s, _, a) in texts_at(&ui) {
            assert!(a > 0.99, "«{s}» is fading in");
        }
    }
    let (p, _) = text_at(&ui, "Fila 5000").expect("row 5000 is in view");
    assert!((p.y - list.y - (30.0 - 15.0 * 1.3) * 0.5).abs() < 1.0, "at the top: {} vs {}", p.y, list.y);
    assert!(text_at(&ui, "Fila 0").is_none() && built.get() <= 11);
    // And the end is where 20 000 rows end.
    ui.scroll_wheel(-1.0e9);
    settle(&mut ui, &mut t, build);
    assert!(text_at(&ui, "Fila 19999").is_some());
}

/// Every text drawn, with its position and opacity.
fn texts_at(ui: &Ui) -> Vec<(String, Vec2, f32)> {
    let list = ui.draw_list();
    list.commands()
        .iter()
        .filter_map(|c| {
            if let DrawCmd::Text { pos, color, start, len, .. } = *c {
                Some((list.text(start, len).to_string(), pos, color.a))
            } else {
                None
            }
        })
        .collect()
}

#[test]
fn cards_far_from_view_are_not_built() {
    let built = Cell::new(0);
    let name = RefCell::new(String::new());
    let build = |ui: &mut Ui| {
        built.set(0);
        ui.scroll("pagina", 400.0, |ui| {
            for i in 0..60 {
                ui.card(&format!("tarjeta-{i}"), |ui| {
                    built.set(built.get() + 1);
                    ui.label(&format!("Tarjeta {i}"));
                    if i == 1 {
                        ui.text_field("nombre", &mut name.borrow_mut(), "Nombre", 200.0);
                    }
                    ui.button("abrir", "Abrir");
                });
            }
        });
    };
    // With and without it, everything is in the same place.
    let (mut eager, mut te) = (Ui::new(Theme::dark()), 0.0);
    eager.set_lazy(false);
    settle(&mut eager, &mut te, build);
    assert_eq!(built.get(), 60);
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    settle(&mut ui, &mut t, build);
    frame(&mut ui, &mut t, build);
    assert!(built.get() < 20, "{} of 60 cards built", built.get());
    assert!(ui.find("tarjeta-50").is_some(), "the card keeps its place");
    assert!(ui.find_all("abrir").len() < 20, "what it holds is not built, so it is not found");

    // Scrolled to the end, the last card is where it would have been, and
    // it shows as it is: nothing fades in.
    for u in [&mut eager, &mut ui] {
        u.pointer_moved(Vec2::new(100.0, 200.0));
        u.scroll_wheel(-1.0e6);
    }
    settle(&mut eager, &mut te, build);
    for _ in 0..300 {
        let moving = frame(&mut ui, &mut t, build);
        for (s, _, a) in texts_at(&ui) {
            assert!(a > 0.99, "«{s}» is fading in");
        }
        if !moving {
            break;
        }
    }
    let last = |u: &Ui| text_at(u, "Tarjeta 59").unwrap().0;
    assert_eq!(last(&ui), last(&eager));
    assert!(built.get() < 20);

    // A card that holds the focus keeps being built, wherever it is.
    ui.scroll_wheel(1.0e6);
    settle(&mut ui, &mut t, build);
    let field = ui.find("nombre").unwrap();
    click(&mut ui, &mut t, field, build);
    ui.scroll_wheel(-1.0e6);
    settle(&mut ui, &mut t, build);
    ui.text_input("Ana");
    frame(&mut ui, &mut t, build);
    assert_eq!(*name.borrow(), "Ana", "typing still reaches the field out of view");
}

// ----- reduced motion ---------------------------------------------------------

use morfo::MotionPreference;

#[test]
fn reduced_motion_is_brief_and_never_overshoots() {
    let run = |pref: MotionPreference| {
        let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
        ui.set_motion(pref);
        let open = Cell::new(false);
        let build = |ui: &mut Ui| {
            ui.reveal("extra", open.get(), |ui| {
                ui.label("uno");
                ui.label("dos");
            });
            ui.button("debajo", "Debajo");
        };
        settle(&mut ui, &mut t, build);
        let start = ui.rect_of(ui.id("debajo")).unwrap().y;
        open.set(true);
        let mut furthest = start;
        let mut frames = 0;
        for _ in 0..600 {
            frames += 1;
            let moving = frame(&mut ui, &mut t, build);
            furthest = furthest.max(ui.rect_of(ui.id("debajo")).unwrap().y);
            if !moving {
                break;
            }
        }
        (frames, furthest - ui.rect_of(ui.id("debajo")).unwrap().y, ui.rect_of(ui.id("debajo")).unwrap().y - start)
    };
    let (full, _, moved) = run(MotionPreference::Full);
    let (reduced, overshoot, moved_r) = run(MotionPreference::Reduced);
    let (off, _, moved_o) = run(MotionPreference::Off);
    assert!(moved > 10.0 && (moved - moved_r).abs() < 0.5 && (moved - moved_o).abs() < 0.5, "same place in the end");
    assert!(reduced * 2 < full, "reduced is brief: {reduced} vs {full} frames");
    assert!(overshoot.abs() < 0.01, "and never goes past: {overshoot}");
    assert!(off <= 2, "off is immediate: {off} frames");
}

#[test]
fn reduced_motion_leaves_out_what_only_decorates() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    ui.set_motion(MotionPreference::Reduced);
    let blue = Color::hex(0x2050e0);
    let view = Cell::new(0usize);
    let build = |ui: &mut Ui| {
        ui.scroll("pagina", 300.0, |ui| {
            ui.space(200.0);
            ui.parallax("paisaje", 120.0, &[0.5], |ui, _| {
                ui.surface("capa", Vec2::new(200.0, 120.0), 0.0, blue);
            });
            ui.space(600.0);
        });
        let v = view.get();
        ui.transition("vistas", v, Transition::Slide, |ui| ui.label(if v == 0 { "Uno" } else { "Dos" }));
    };
    settle(&mut ui, &mut t, build);
    // Parallax: the layer moves with the page.
    let layer = |ui: &Ui| rects_of(ui, blue)[0].y;
    let window = |ui: &Ui| ui.rect_of(ui.find("paisaje").unwrap()).unwrap().y;
    let gap = layer(&ui) - window(&ui);
    ui.pointer_moved(ui.rect_of(ui.id("pagina")).unwrap().center());
    ui.scroll_wheel(-100.0);
    settle(&mut ui, &mut t, build);
    assert!((layer(&ui) - window(&ui) - gap).abs() < 0.5, "no depth");
    // Past the end of the scroll: no bounce.
    let top = window(&ui);
    ui.scroll_wheel(-1.0e5);
    let mut lowest = f32::MAX;
    for _ in 0..200 {
        let moving = frame(&mut ui, &mut t, build);
        lowest = lowest.min(window(&ui));
        if !moving {
            break;
        }
    }
    assert!(window(&ui) - lowest < 0.5 && window(&ui) < top, "it stops at the end without stretching");
    // A slide between views becomes a cross-fade: nothing travels sideways.
    let home = text_at(&ui, "Uno").unwrap().0;
    view.set(1);
    for _ in 0..60 {
        frame(&mut ui, &mut t, build);
        for s in ["Uno", "Dos"] {
            if let Some((p, _)) = text_at(&ui, s) {
                assert!((p.x - home.x).abs() < 0.5, "«{s}» moved sideways to {}", p.x);
            }
        }
    }
}

#[test]
fn a_loading_skeleton_does_not_hold_every_frame() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let build = |ui: &mut Ui| ui.skeleton("perfil", true, 3, |ui| ui.label("Cargado"));
    settle(&mut ui, &mut t, build);
    // At rest between shimmer steps, asking to be woken at 30 fps.
    t += 1.0 / 60.0;
    ui.begin(t, VIEW);
    build(&mut ui);
    let out = ui.end();
    assert!(!out.animating);
    let wake = out.wake_in.expect("the shimmer asks for its next step");
    assert!((wake - 1.0 / 30.0).abs() < 1e-3, "{wake}");
    // With reduced motion it is still and asks for nothing.
    ui.set_motion(MotionPreference::Reduced);
    settle(&mut ui, &mut t, build);
    t += 1.0;
    ui.begin(t, VIEW);
    build(&mut ui);
    assert!(ui.end().wake_in.is_none());
}

// ----- the components' own words ---------------------------------------------

#[test]
fn the_components_speak_the_chosen_language() {
    use morfo::{Date, Locale};
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let date = RefCell::new(Date::new(2026, 11, 16));
    let build = |ui: &mut Ui| {
        ui.date_picker("fecha", &mut date.borrow_mut());
        ui.count_up("n", 12480.5, 1, 20.0);
    };
    settle(&mut ui, &mut t, build);
    assert!(texts(&ui).contains(&"Nov 16, 2026".to_string()), "{:?}", texts(&ui));
    assert!(texts(&ui).contains(&"12,480.5".to_string()));
    // The calendar starts its weeks on Sunday in English.
    let fecha = ui.find("fecha").unwrap();
    click(&mut ui, &mut t, fecha, build);
    settle(&mut ui, &mut t, build);
    let shown = texts(&ui);
    let su = shown.iter().position(|s| s == "Su").unwrap();
    assert_eq!(shown[su + 1], "Mo");
    assert!(shown.contains(&"November 2026".to_string()));
    ui.key_pressed(Key::Escape);
    frame(&mut ui, &mut t, build);

    ui.set_locale(Locale::SPANISH);
    settle(&mut ui, &mut t, build);
    assert!(texts(&ui).contains(&"16 nov 2026".to_string()));
    assert!(texts(&ui).contains(&"12.480,5".to_string()));
}

// ----- images -------------------------------------------------------------------

#[test]
fn images_are_registered_shown_replaced_and_removed() {
    use morfo::{Fit, Role};
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    assert!(ui.add_image(2, 2, vec![0; 15]).is_none(), "the bytes must match the size");
    let id = ui.add_image(4, 2, vec![255; 4 * 2 * 4]).unwrap();
    assert_eq!(ui.image_size(id), Some(Vec2::new(4.0, 2.0)));
    let build = |ui: &mut Ui| ui.image("foto", id, Vec2::new(100.0, 100.0), Fit::Cover, "Una foto");
    ui.set_accessibility(true);
    settle(&mut ui, &mut t, build);
    let image = |ui: &Ui| {
        ui.draw_list().commands().iter().find_map(|c| {
            if let DrawCmd::Image { rect, uv, version, tint, .. } = *c {
                Some((rect, uv, version, tint))
            } else {
                None
            }
        })
    };
    let (rect, uv, version, tint) = image(&ui).expect("drawn");
    assert_eq!((rect.w, rect.h), (100.0, 100.0));
    // A 2:1 picture in a square box: half of its width shows, centered.
    assert_eq!(uv, morfo::Rect::new(0.25, 0.0, 0.5, 1.0));
    assert!(tint.a > 0.99);
    assert!(ui.access_nodes().iter().any(|n| n.role == Role::Image && n.label == "Una foto"));
    // New pixels: a new version, so every backend uploads them again.
    assert!(ui.replace_image(id, 2, 2, vec![0; 16]));
    frame(&mut ui, &mut t, build);
    assert_eq!(image(&ui).unwrap().2, version + 1);
    assert_eq!(ui.draw_list().image(id).unwrap().width, 2);
    ui.remove_image(id);
    frame(&mut ui, &mut t, build);
    assert!(image(&ui).is_none(), "a removed image is not drawn");
    assert!(!ui.replace_image(id, 2, 2, vec![0; 16]));
}

// ----- touch and scrolling ------------------------------------------------------

/// A finger from `from` to `to` in `steps` frames, then lifted.
fn swipe(ui: &mut Ui, t: &mut f64, from: Vec2, to: Vec2, steps: u32, mut f: impl FnMut(&mut Ui)) {
    ui.touch_started(from);
    frame(ui, t, &mut f);
    for k in 1..=steps {
        ui.touch_moved(from + (to - from) * (k as f32 / steps as f32));
        frame(ui, t, &mut f);
    }
    ui.touch_ended(to, false);
    frame(ui, t, &mut f);
}

#[test]
fn a_finger_scrolls_flings_and_taps() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let clicks = Cell::new(0);
    let build = |ui: &mut Ui| {
        ui.scroll("lista", 300.0, |ui| {
            for i in 0..60 {
                if ui.button(&format!("b{i}"), &format!("Fila {i}")).clicked {
                    clicks.set(clicks.get() + 1);
                }
            }
        });
    };
    settle(&mut ui, &mut t, build);
    let row = |ui: &Ui| ui.rect_of(ui.find("b2").unwrap()).unwrap().y;
    let start = row(&ui);
    let on = ui.rect_of(ui.find("b3").unwrap()).unwrap().center();
    // A drag that starts on a button scrolls, and does not press it.
    swipe(&mut ui, &mut t, on, on - Vec2::new(0.0, 120.0), 6, build);
    assert_eq!(clicks.get(), 0, "a drag is not a click");
    let after_drag = row(&ui);
    // It follows the finger from the moment the drag is recognized (the
    // first step, 20 px), without a jump.
    assert!(start - after_drag > 95.0, "the content followed the finger: {start} → {after_drag}");
    // Let go fast: it keeps going on its own, then rests.
    settle(&mut ui, &mut t, build);
    assert!(row(&ui) < after_drag - 20.0, "flung further: {after_drag} → {}", row(&ui));
    // A tap is a click, and leaves no hover behind: the row in the middle
    // of the list.
    let list = ui.rect_of(ui.id("lista")).unwrap();
    let middle = (0..60)
        .filter_map(|i| ui.find(&format!("b{i}")))
        .min_by(|&a, &b| {
            let d = |id| (ui.rect_of(id).unwrap().center().y - list.center().y).abs();
            d(a).total_cmp(&d(b))
        })
        .unwrap();
    let b = ui.rect_of(middle).unwrap().center();
    ui.touch_started(b);
    frame(&mut ui, &mut t, build);
    ui.touch_ended(b, false);
    frame(&mut ui, &mut t, build);
    assert_eq!(clicks.get(), 1);
    frame(&mut ui, &mut t, build);
    // A finger is broad: a tap just past the side of the button still hits it.
    let r = ui.rect_of(middle).unwrap();
    let edge = Vec2::new(r.right() + 4.0, r.center().y);
    {
        ui.touch_started(edge);
        frame(&mut ui, &mut t, build);
        ui.touch_ended(edge, false);
        frame(&mut ui, &mut t, build);
        assert_eq!(clicks.get(), 2);
    }
}

#[test]
fn a_long_press_is_not_a_click() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let (clicks, long) = (Cell::new(0), Cell::new(0));
    let build = |ui: &mut Ui| {
        let r = ui.button("b", "Mantén");
        clicks.set(clicks.get() + r.clicked as u32);
        long.set(long.get() + r.long_pressed as u32);
    };
    settle(&mut ui, &mut t, build);
    let at = ui.rect_of(ui.id("b")).unwrap().center();
    ui.touch_started(at);
    for _ in 0..40 {
        frame(&mut ui, &mut t, build);
    }
    ui.touch_ended(at, false);
    frame(&mut ui, &mut t, build);
    assert_eq!((long.get(), clicks.get()), (1, 0), "reported once, and the release is not a click");
}

#[test]
fn the_scroll_bar_drags_and_page_keys_and_focus_move_the_view() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let build = |ui: &mut Ui| {
        ui.scroll("lista", 300.0, |ui| {
            for i in 0..60 {
                ui.button(&format!("b{i}"), &format!("Fila {i}"));
            }
        });
    };
    settle(&mut ui, &mut t, build);
    let area = ui.rect_of(ui.id("lista")).unwrap();
    let row0 = |ui: &Ui| ui.rect_of(ui.find("b0").unwrap()).unwrap().y;
    let top = row0(&ui);
    // The bar, dragged to the bottom: the content goes to its end.
    let grab = Vec2::new(area.right() - 4.0, area.y + 10.0);
    drag(&mut ui, &mut t, grab, Vec2::new(grab.x, area.bottom() + 50.0), build);
    settle(&mut ui, &mut t, build);
    assert!(text_at(&ui, "Fila 59").is_some(), "dragged to the end");
    // Page Up, with the pointer over the area: a screen back.
    let before = row0(&ui);
    ui.pointer_moved(area.center());
    ui.key_pressed(Key::PageUp);
    settle(&mut ui, &mut t, build);
    assert!(row0(&ui) - before > 200.0, "a page up: {before} → {}", row0(&ui));
    // Back at the top, Tab through the rows: the view follows the focus.
    ui.key_pressed(Key::Home);
    ui.scroll_wheel(1.0e6);
    settle(&mut ui, &mut t, build);
    assert!((row0(&ui) - top).abs() < 0.5);
    ui.pointer_left();
    for _ in 0..15 {
        ui.key_pressed(Key::Tab);
        frame(&mut ui, &mut t, build);
    }
    settle(&mut ui, &mut t, build);
    let focused = ui.rect_of(ui.focused().unwrap()).unwrap();
    assert!(focused.y >= area.y && focused.bottom() <= area.bottom(), "the focused row is in view: {focused:?} in {area:?}");
}

// ----- adaptation policy, text size, retention ---------------------------------

#[test]
fn the_adaptation_policy_decides_when_the_layout_moves() {
    use morfo::Adaptation;
    let run = |policy: Adaptation, request: bool| {
        let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
        ui.set_adaptation(policy);
        let build = |ui: &mut Ui| {
            ui.adaptive_actions("bar", &ITEMS);
            ui.suggestions("quick", "Quick:", "bar", &ITEMS, 2);
        };
        settle(&mut ui, &mut t, build);
        let c = ui.id("bar").child("c");
        for _ in 0..6 {
            click(&mut ui, &mut t, c, build);
        }
        ui.pointer_left();
        if request {
            ui.adapt_now();
        }
        settle(&mut ui, &mut t, build);
        let moved = ui.rect_of(c).unwrap().x < ui.rect_of(ui.id("bar").child("a")).unwrap().x;
        let suggested = ui.rect_of(ui.id("quick").child("c")).is_some();
        (moved, suggested)
    };
    assert_eq!(run(Adaptation::Live, false), (true, true));
    assert_eq!(run(Adaptation::OnRequest, false), (false, true), "stable until asked");
    assert_eq!(run(Adaptation::OnRequest, true), (true, true));
    assert_eq!(run(Adaptation::Suggest, true), (false, true), "only suggestions");
    assert_eq!(run(Adaptation::Off, true), (false, false), "nothing");
}

#[test]
fn larger_text_reflows_and_controls_grow_to_fit() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let build = |ui: &mut Ui| {
        ui.paragraph("Un párrafo bastante largo que a doble tamaño necesita bastantes más líneas para caber en el mismo ancho.");
        ui.button("ok", "Aceptar");
    };
    settle(&mut ui, &mut t, build);
    let lines = |ui: &Ui| texts(ui).len();
    let (before, button) = (lines(&ui), ui.rect_of(ui.id("ok")).unwrap());
    ui.set_text_scale(2.0);
    settle(&mut ui, &mut t, build);
    assert!(lines(&ui) > before, "more lines: {before} → {}", lines(&ui));
    let bigger = ui.rect_of(ui.id("ok")).unwrap();
    assert!(bigger.h > button.h * 1.3 && bigger.w > button.w * 1.6, "{button:?} → {bigger:?}");
}

#[test]
fn a_retention_period_forgets_old_habits() {
    let mut p = morfo::Profile::new();
    p.record(morfo::Id(1), 0.0);
    p.record(morfo::Id(2), 100.0);
    p.prune(150.0, 120.0);
    assert_eq!(p.count(morfo::Id(1)), 0, "used 150 s ago: forgotten");
    assert_eq!(p.count(morfo::Id(2)), 1);
}

#[test]
fn screen_readers_get_a_tree_with_states_and_announcements() {
    use morfo::Role;
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    ui.set_accessibility(true);
    let open = Cell::new(Some(0usize));
    let build = |ui: &mut Ui| {
        ui.card("tarjeta", |ui| {
            ui.button("ok", "Aceptar");
            ui.progress(0.4, 100.0);
        });
        let mut o = open.get();
        ui.accordion("faq", &["Uno", "Dos"], &mut o, |ui, _| ui.label("Cuerpo"));
        open.set(o);
    };
    ui.toast("Guardado");
    settle(&mut ui, &mut t, build);
    // Settle leaves the toast gone; build a frame while it is still up.
    let mut ui2 = Ui::new(Theme::dark());
    ui2.set_accessibility(true);
    ui2.begin(0.0, VIEW);
    ui2.toast("Guardado");
    ui2.end();
    ui2.begin(0.1, VIEW);
    build(&mut ui2);
    ui2.end();
    let nodes = ui2.access_nodes();
    let card = nodes.iter().find(|n| n.role == Role::Group).expect("the card is a group");
    let ok = nodes.iter().find(|n| n.label == "ok" || n.label == "Aceptar").unwrap();
    assert_eq!(ok.parent, Some(card.id), "the button is inside the card");
    let bar = nodes.iter().find(|n| n.role == Role::ProgressBar).expect("the progress bar is described");
    assert_eq!(bar.value.as_deref(), Some("40 %"));
    let headers: Vec<_> = nodes.iter().filter(|n| n.label == "Uno" || n.label == "Dos").collect();
    assert_eq!(headers.iter().map(|n| n.expanded).collect::<Vec<_>>(), [Some(true), Some(false)]);
    let toast = nodes.iter().find(|n| n.label == "Guardado").expect("the toast is described");
    assert!(toast.role == Role::Status && toast.live, "and announced");
    let _ = (&mut ui, &mut t);
}

// ----- text editing ---------------------------------------------------------

#[test]
fn undo_redo_words_and_double_click() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let text = RefCell::new(String::new());
    let build = |ui: &mut Ui| {
        ui.text_field("campo", &mut text.borrow_mut(), "", 400.0);
    };
    settle(&mut ui, &mut t, build);
    let field = ui.id("campo");
    click(&mut ui, &mut t, field, build);
    // Typed in one go: one step to undo.
    for w in ["hola", " ", "mundo"] {
        ui.text_input(w);
        frame(&mut ui, &mut t, build);
    }
    t += 2.0; // a pause: what comes next is another step
    ui.text_input(" bonito");
    frame(&mut ui, &mut t, build);
    assert_eq!(*text.borrow(), "hola mundo bonito");
    ui.key_pressed(Key::Undo);
    frame(&mut ui, &mut t, build);
    assert_eq!(*text.borrow(), "hola mundo");
    ui.key_pressed(Key::Undo);
    frame(&mut ui, &mut t, build);
    assert_eq!(*text.borrow(), "");
    ui.key_pressed(Key::Redo);
    frame(&mut ui, &mut t, build);
    assert_eq!(*text.borrow(), "hola mundo");
    // Ctrl+Backspace takes the whole last word.
    ui.set_modifiers(morfo::Modifiers { ctrl: true, shift: false });
    ui.key_pressed(Key::Backspace);
    frame(&mut ui, &mut t, build);
    ui.set_modifiers(morfo::Modifiers::default());
    assert_eq!(*text.borrow(), "hola ");
    // A double click selects the word under it; typing replaces it.
    let r = ui.rect_of(ui.id("campo")).unwrap();
    let on = Vec2::new(r.x + 20.0, r.center().y);
    for _ in 0..2 {
        ui.pointer_moved(on);
        ui.pointer_pressed();
        frame(&mut ui, &mut t, build);
        ui.pointer_released();
        frame(&mut ui, &mut t, build);
    }
    ui.text_input("adiós");
    frame(&mut ui, &mut t, build);
    assert_eq!(*text.borrow(), "adiós ");
}

#[test]
fn passwords_hide_and_numbers_stay_numbers() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    ui.set_accessibility(true);
    let (pass, num) = (RefCell::new(String::new()), Cell::new(5.0f64));
    let build = |ui: &mut Ui| {
        ui.password_field("clave", &mut pass.borrow_mut(), "Contraseña", 200.0);
        let mut n = num.get();
        ui.number_field("cantidad", &mut n, 1, (0.0, 10.0), 120.0);
        num.set(n);
    };
    settle(&mut ui, &mut t, build);
    let clave = ui.id("clave");
    click(&mut ui, &mut t, clave, build);
    ui.text_input("secreto");
    frame(&mut ui, &mut t, build);
    assert_eq!(*pass.borrow(), "secreto");
    assert!(!texts(&ui).iter().any(|s| s.contains("secreto")), "the password is not drawn");
    assert!(texts(&ui).contains(&"•••••••".to_string()));
    ui.key_pressed(Key::SelectAll);
    ui.key_pressed(Key::Copy);
    frame(&mut ui, &mut t, build);
    assert_eq!(ui.take_clipboard(), None, "a password is never copied");
    assert!(!ui.access_nodes().iter().any(|n| n.value.as_deref() == Some("secreto")), "nor read out");

    let field = ui.id("cantidad");
    click(&mut ui, &mut t, field, build);
    ui.key_pressed(Key::Up);
    frame(&mut ui, &mut t, build);
    assert!((num.get() - 5.1).abs() < 1e-9, "Up steps one decimal: {}", num.get());
    ui.key_pressed(Key::SelectAll);
    frame(&mut ui, &mut t, build);
    ui.text_input("4x2.5");
    frame(&mut ui, &mut t, build);
    assert!((num.get() - 42.5).abs() < 1e-9, "letters are not taken: {}", num.get());
    ui.key_pressed(Key::Enter);
    frame(&mut ui, &mut t, build);
    assert_eq!(num.get(), 10.0, "Enter brings it within its range");
    frame(&mut ui, &mut t, build);
    assert!(texts(&ui).contains(&"10.0".to_string()), "and writes it out: {:?}", texts(&ui));
}

// ----- rich text --------------------------------------------------------------------

/// A paragraph of styled spans keeps each span's style, wraps as one text
/// and is read as one sentence.
#[test]
fn rich_text_keeps_styles_and_wraps_as_one() {
    use morfo::{Span, TextStyle};
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    ui.set_accessibility(true);
    let long = "word ".repeat(60);
    let spans = [Span::new("Press "), Span::new("Save").bold(), Span::new(" or "), Span::new("Ctrl+S").mono(), Span::new(&long).italic()];
    settle(&mut ui, &mut t, |ui| {
        ui.rich(&spans);
    });
    let list = ui.draw_list();
    let runs: Vec<(String, TextStyle, f32, f32)> = list
        .commands()
        .iter()
        .filter_map(|c| {
            if let DrawCmd::Text { pos, start, len, style, .. } = *c {
                Some((list.text(start, len).to_string(), style, pos.x, pos.y))
            } else {
                None
            }
        })
        .collect();
    let find = |s: &str| runs.iter().find(|r| r.0.starts_with(s)).unwrap_or_else(|| panic!("{s:?} missing in {runs:?}"));
    assert_eq!(find("Press").1, TextStyle::REGULAR);
    assert_eq!(find("Save").1, TextStyle::BOLD);
    assert_eq!(find("Ctrl+S").1, TextStyle::MONO);
    assert_eq!(find("word").1, TextStyle::ITALIC);
    // One line flows across spans; the long one wraps onto more lines.
    assert!(find("Save").2 > find("Press").2 && find("Save").3 == find("Press").3);
    let lines: std::collections::BTreeSet<u32> = runs.iter().map(|r| r.3.to_bits()).collect();
    assert!(lines.len() > 2, "the paragraph wraps: {lines:?}");
    let labels: Vec<&str> = ui.access_nodes().iter().map(|n| n.label.as_str()).collect();
    assert!(labels.iter().any(|l| l.starts_with("Press Save or Ctrl+Sword word")), "{labels:?}");
}

// ----- right to left ----------------------------------------------------------------

/// Written right to left, the interface is a mirror image: what starts at
/// the left starts at the right, clicks land where things are shown,
/// Hebrew reads from the right, and a text field keeps its caret at the
/// end of what was typed.
#[test]
fn right_to_left_mirrors_the_interface() {
    use morfo::Locale;
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    ui.set_locale(Locale::ENGLISH.with_rtl(true));
    let (clicks, text) = (std::cell::Cell::new(0), std::cell::RefCell::new(String::new()));
    let build = |ui: &mut Ui| {
        if ui.button("go", "שלום").clicked {
            clicks.set(clicks.get() + 1);
        }
        ui.text_field("f", &mut text.borrow_mut(), "Name", 200.0);
    };
    settle(&mut ui, &mut t, build);
    let go = ui.rect_of(ui.id("go")).unwrap();
    assert!(go.right() > VIEW.x - 40.0 && go.x > VIEW.x / 2.0, "the first button sits at the right: {go:?}");
    let go = ui.id("go");
    click(&mut ui, &mut t, go, build);
    assert_eq!(clicks.get(), 1, "a click where it is shown reaches it");
    let list = ui.draw_list();
    let texts: Vec<(String, f32)> = list
        .commands()
        .iter()
        .filter_map(|c| if let DrawCmd::Text { pos, start, len, .. } = *c { Some((list.text(start, len).to_string(), pos.x)) } else { None })
        .collect();
    assert!(texts.iter().any(|(s, _)| s == "םולש"), "Hebrew in reading order: {texts:?}");
    // Type into the field: the caret ends up right after the text.
    let f = ui.id("f");
    click(&mut ui, &mut t, f, build);
    ui.text_input("abc");
    settle(&mut ui, &mut t, build);
    assert_eq!(*text.borrow(), "abc");
    let list = ui.draw_list();
    let (x, _) = list
        .commands()
        .iter()
        .find_map(|c| if let DrawCmd::Text { pos, start, len, .. } = *c { (list.text(start, len) == "abc").then_some((pos.x, ())) } else { None })
        .unwrap();
    let field = ui.rect_of(f).unwrap();
    assert!(x > field.x && x < field.x + 30.0, "the field's text starts at its left, in its own order: {x} in {field:?}");
    let ime = ui.ime_area().unwrap();
    assert!(ime.x > x && ime.x < x + 60.0, "the caret follows the text: {ime:?} after {x}");
}

/// A gamepad or remote moves the focus to the nearest control that way.
#[test]
fn focus_moves_spatially() {
    use morfo::Direction;
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let build = |ui: &mut Ui| {
        ui.row(|ui| {
            ui.button("a", "A");
            ui.button("b", "B");
        });
        ui.row(|ui| {
            ui.button("c", "C");
            ui.button("d", "D");
        });
    };
    settle(&mut ui, &mut t, build);
    let id = |ui: &Ui, k: &str| ui.find(k).unwrap();
    let mut step = |ui: &mut Ui, d: Direction| {
        ui.move_focus(d);
        frame(ui, &mut t, build);
        ui.focused()
    };
    assert_eq!(step(&mut ui, Direction::Down), Some(id(&ui, "a")), "nothing focused: the first control");
    assert_eq!(step(&mut ui, Direction::Right), Some(id(&ui, "b")));
    assert_eq!(step(&mut ui, Direction::Down), Some(id(&ui, "d")));
    assert_eq!(step(&mut ui, Direction::Left), Some(id(&ui, "c")));
    assert_eq!(step(&mut ui, Direction::Left), Some(id(&ui, "c")), "nothing further: it stays");
    assert_eq!(step(&mut ui, Direction::Up), Some(id(&ui, "a")));
}

/// Something dragged onto a target lands there; let go elsewhere, nothing
/// happens; and the drag is no click.
#[test]
fn drag_and_drop() {
    let (mut ui, mut t) = (Ui::new(Theme::dark()), 0.0);
    let (dropped, clicked) = (std::cell::Cell::new(None), std::cell::Cell::new(false));
    let build = |ui: &mut Ui| {
        if ui
            .draggable("card", 7, |ui| {
                // A control inside: the drag takes the press from it.
                if ui.button("inner", "Card").clicked {
                    clicked.set(true);
                }
            })
            .clicked
        {
            clicked.set(true);
        }
        ui.space(80.0);
        if let Some(p) = ui.drop_target("bin", |ui, _| ui.label("Drop here")) {
            dropped.set(Some(p));
        }
    };
    settle(&mut ui, &mut t, build);
    let (card, bin) = (ui.rect_of(ui.find("card").unwrap()).unwrap(), ui.rect_of(ui.find("bin").unwrap()).unwrap());
    ui.pointer_moved(card.center());
    frame(&mut ui, &mut t, build);
    ui.pointer_pressed();
    frame(&mut ui, &mut t, build);
    // Along the way, a few frames.
    for k in 1..=6 {
        ui.pointer_moved(card.center() + (bin.center() - card.center()) * (k as f32 / 6.0));
        frame(&mut ui, &mut t, build);
    }
    assert_eq!(ui.dragging(), Some(7));
    ui.pointer_released();
    frame(&mut ui, &mut t, build);
    assert_eq!(dropped.get(), Some(7));
    assert!(!clicked.get());
    assert_eq!(ui.dragging(), None);
    // Dragged and let go away from the target: nothing.
    dropped.set(None);
    ui.pointer_moved(card.center());
    frame(&mut ui, &mut t, build);
    ui.pointer_pressed();
    frame(&mut ui, &mut t, build);
    ui.pointer_moved(card.center() + Vec2::new(300.0, 0.0));
    frame(&mut ui, &mut t, build);
    ui.pointer_released();
    frame(&mut ui, &mut t, build);
    assert_eq!(dropped.get(), None);
    // A finger: held still first, then carried over.
    settle(&mut ui, &mut t, build);
    ui.touch_started(card.center());
    for _ in 0..40 {
        frame(&mut ui, &mut t, build);
    }
    assert_eq!(ui.dragging(), Some(7), "a long press picks it up");
    for k in 1..=6 {
        ui.touch_moved(card.center() + (bin.center() - card.center()) * (k as f32 / 6.0));
        frame(&mut ui, &mut t, build);
    }
    ui.touch_ended(bin.center(), false);
    frame(&mut ui, &mut t, build);
    assert_eq!(dropped.get(), Some(7));
    assert!(!clicked.get());
}

/// The test harness drives an interface by keys and reads it back; the
/// inspector names what is under the pointer.
#[test]
fn harness_and_inspector() {
    use morfo::testing::Harness;
    let mut h = Harness::new();
    let mut name = String::new();
    let mut saved = false;
    let mut app = |ui: &mut Ui| {
        ui.card("form", |ui| {
            ui.text_field("name", &mut name, "Name", 240.0);
            if ui.button("save", "Save").clicked {
                saved = true;
            }
        });
        ui.label(if saved { "Saved" } else { "Unsaved" });
    };
    h.settle(&mut app);
    assert!(h.shows("Unsaved"));
    h.type_into("name", "Ada", &mut app);
    h.click("save", &mut app);
    assert!(h.shows("Saved") && !h.shows("Unsaved"));
    assert!(h.node("Save").is_some_and(|n| n.role == morfo::Role::Button));
    drop(app);
    assert_eq!(name, "Ada");
    assert!(saved);

    let mut h = Harness::new();
    h.ui.set_inspector(true);
    let mut app = |ui: &mut Ui| {
        ui.button("ok", "OK");
    };
    h.settle(&mut app);
    let at = h.ui.rect_of(h.id("ok")).unwrap().center();
    h.ui.pointer_moved(at);
    h.frame(&mut app);
    assert!(h.texts().iter().any(|t| t.starts_with("ok  ")), "{:?}", h.texts());
    assert!(h.shows("elements ·"));
}

/// A form field names its group and announces its error.
#[test]
fn form_fields_announce_errors() {
    use morfo::testing::Harness;
    for theme in [Theme::dark(), Theme::light()] {
        let mut h = Harness::with(theme, VIEW);
        let mut email = String::from("ada");
        let mut app = |ui: &mut Ui| {
            let error = (!email.contains('@')).then_some("An email address has an @");
            ui.field("email", "Email", error, |ui| {
                ui.text_field("input", &mut email, "you@example.com", 280.0);
            });
        };
        h.settle(&mut app);
        let status = h.node("An email address has an @").expect("the error is described");
        assert!(status.live && status.role == morfo::Role::Status);
        let group = h.ui.access_nodes().iter().find(|n| n.label == "Email" && n.role == morfo::Role::Group).expect("a group named by the label");
        assert!(h.ui.access_nodes().iter().filter(|n| n.parent == Some(group.id)).count() >= 2, "label, input and error inside it");
        let list = h.ui.draw_list();
        let red = list
            .commands()
            .iter()
            .find_map(|c| {
                if let DrawCmd::Text { color, start, len, .. } = *c {
                    (list.text(start, len).starts_with("An email")).then_some(color)
                } else {
                    None
                }
            })
            .unwrap();
        assert!(morfo::contrast(red, h.ui.theme().palette[morfo::theme::BG]) >= 4.5, "readable error");
        h.type_into("input", "@x.org", &mut app);
        assert!(h.node("An email address has an @").is_none());
    }
}
