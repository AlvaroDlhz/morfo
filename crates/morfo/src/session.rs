//! Picking an interface up where it was left: after a relaunch (a new
//! session, or a live reload during development) the application restores
//! its own state, and this restores what only the [`Ui`] knows.

use crate::id::Id;
use crate::ui::Ui;

const HEADER: &str = "morfo-ui1";

/// Frames a restored scroll position is held while the layout settles.
const HOLD: u8 = 3;

impl Ui {
    /// The interface state that is not the application's: where every scroll
    /// area is. Text, to be given to [`restore_state`](Self::restore_state)
    /// by the next run. (What was learned about the user is saved apart,
    /// with [`profile`](Self::profile).)
    pub fn save_state(&self) -> String {
        let mut rows: Vec<(Id, f32)> = self.scroll_ids.iter().filter_map(|id| Some((*id, self.nodes.get(id)?.aux.target()))).collect();
        // Areas restored but not shown since: still as they were saved.
        rows.extend(self.scroll_restore.iter().map(|(id, (offset, _))| (*id, *offset)));
        rows.sort_by_key(|(id, _)| *id);
        rows.dedup_by_key(|(id, _)| *id);
        let mut out = format!("{HEADER}\n");
        for (id, offset) in rows.into_iter().filter(|(_, offset)| *offset > 0.0) {
            out.push_str(&format!("s {:016x} {offset}\n", id.0));
        }
        out
    }

    /// Inverse of [`save_state`](Self::save_state): each scroll area goes
    /// back to its position the first time it is shown, as far as its
    /// content still reaches. Returns `false` on text it cannot read.
    pub fn restore_state(&mut self, text: &str) -> bool {
        let mut lines = text.lines();
        if lines.next() != Some(HEADER) {
            return false;
        }
        for line in lines {
            let mut fields = line.split(' ');
            // Kinds of line added by later versions are skipped.
            if fields.next() != Some("s") {
                continue;
            }
            let id = fields.next().and_then(|f| u64::from_str_radix(f, 16).ok());
            let offset = fields.next().and_then(|f| f.parse::<f32>().ok()).filter(|o| o.is_finite());
            let (Some(id), Some(offset)) = (id, offset) else { return false };
            self.scroll_restore.insert(Id(id), (offset, HOLD));
        }
        true
    }

    /// While on, every spring jumps straight to its target and nothing
    /// animates. Run a few frames like this, without painting them, to bring
    /// a restored interface to rest before it is first seen: no entrance
    /// animations, the layout already measured, the scroll where it was.
    pub fn set_instant(&mut self, instant: bool) {
        self.instant = instant;
    }
}
