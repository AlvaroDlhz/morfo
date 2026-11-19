//! A dashboard with its own title bar: tabs, charts that travel to new
//! data, a gauge, and a light or dark theme.
//!
//! `cargo run -p morfo-winit --example dashboard`

use morfo::{Theme, Vec2};
use morfo_winit::Options;

const MONTHS: [&str; 6] = ["Jun", "Jul", "Aug", "Sep", "Oct", "Nov"];
const SALES: [[f32; 6]; 2] = [[120.0, 95.0, 140.0, 180.0, 160.0, 210.0], [80.0, 110.0, 90.0, 130.0, 170.0, 150.0]];

fn main() -> Result<(), morfo_winit::Error> {
    let (mut year, mut light, mut tab) = (0usize, false, 0usize);
    let options = Options { decorations: false, size: Vec2::new(960.0, 720.0), ..Options::new("Dashboard") };
    morfo_winit::run(options, move |ui, cx| {
        let maximized = cx.window().is_maximized();
        let bar = ui.title_bar("bar", "Dashboard", maximized, |ui| {
            ui.fill();
            if ui.toggle("light", &mut light) {
                ui.set_theme(if light { Theme::light() } else { Theme::dark() });
            }
        });
        cx.title_bar(bar);
        ui.tabs("sections", &["Sales", "Visits"], &mut tab);
        ui.segmented("year", &["This year", "Last year"], &mut year);
        let width = ui.available_width();
        ui.card("chart", |ui| {
            if tab == 0 {
                ui.bar_chart("sales", &MONTHS, &SALES[year], Vec2::new(width - 40.0, 260.0));
            } else {
                let series: [(&str, &[f32]); 2] = [("This year", &SALES[0]), ("Last year", &SALES[1])];
                ui.line_chart("visits", &MONTHS, &series, Vec2::new(width - 40.0, 260.0));
            }
        });
        ui.row(|ui| {
            let goal = SALES[year].iter().sum::<f32>() / 1200.0;
            ui.gauge("goal", "Yearly goal", goal.min(1.0), 140.0);
            ui.column(|ui| {
                ui.label("Trend");
                ui.sparkline(&SALES[year], 220.0);
            });
        });
    })
}
