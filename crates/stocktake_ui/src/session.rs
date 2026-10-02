//! The stocktake in progress, as an entity: the counts so far, and whether
//! they've reached the disk.
//!
//! Every count is saved the moment it's recorded, so the save state lives
//! here with the data rather than in a view: a stocktake that looks saved is
//! saved. Views read it, record counts through it, and hear about a failed
//! save through [`SessionEvent`].

use std::{io, path::PathBuf};

use gpui_kit::{Context, EventEmitter, SharedString};
use stocktake::{ProductId, Stocktake, store};

/// Whether the last change reached the disk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SaveState {
    Saved,
    Failed,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SessionEvent {
    /// A change wasn't saved, for the reason given. It's kept in memory, and
    /// the next successful save writes it too.
    SaveFailed(SharedString),
}

pub(crate) struct Session {
    stocktake: Stocktake,
    /// Where the stocktake is saved. Only one stocktake exists at a time, so
    /// this is the same file for every session.
    store_path: PathBuf,
    save_state: SaveState,
}

impl EventEmitter<SessionEvent> for Session {}

impl Session {
    /// A session over `stocktake`, saved to `store_path`. Nothing is written
    /// until [`Self::save`] or a count: a resumed stocktake is already on
    /// disk, and a new one is saved by whoever imported it.
    pub fn new(stocktake: Stocktake, store_path: PathBuf) -> Self {
        Self {
            stocktake,
            store_path,
            save_state: SaveState::Saved,
        }
    }

    pub fn stocktake(&self) -> &Stocktake {
        &self.stocktake
    }

    pub fn save_state(&self) -> SaveState {
        self.save_state
    }

    /// Records what was counted of a product at `location`, replacing any
    /// earlier count there, and saves at once. With `moves_pick_location`,
    /// `location` becomes the product's pick location first.
    pub fn record_count(
        &mut self,
        id: ProductId,
        location: &str,
        quantity: i64,
        moves_pick_location: bool,
        cx: &mut Context<Self>,
    ) {
        if moves_pick_location {
            self.stocktake.move_pick_location(id, location);
        }
        self.stocktake.set_count(id, location, quantity);
        self.save(cx);
    }

    /// Writes the stocktake to disk. A write that fails marks the session
    /// unsaved and emits [`SessionEvent::SaveFailed`].
    pub fn save(&mut self, cx: &mut Context<Self>) {
        self.save_state = match store::save(&self.store_path, &self.stocktake) {
            Ok(()) => SaveState::Saved,
            Err(error) => {
                cx.emit(SessionEvent::SaveFailed(error.to_string().into()));
                SaveState::Failed
            }
        };
        cx.notify();
    }

    /// Deletes the saved stocktake, so the next start has none to resume.
    /// The session itself is left as it was; drop it afterwards.
    pub fn discard(&self) -> io::Result<()> {
        store::clear(&self.store_path)
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use gpui_kit::{AppContext as _, TestAppContext};
    use stocktake::Product;

    use super::*;

    fn stocktake() -> Stocktake {
        Stocktake::new(vec![Product::new("1", "Vare", "A1", "", 5)])
    }

    #[gpui_kit::test]
    fn a_count_is_saved_at_once(cx: &mut TestAppContext) {
        let dir = std::env::temp_dir().join(format!("stocktake-session-{}", std::process::id()));
        let path = dir.join("varetelling.json");
        let session = cx.new(|_| Session::new(stocktake(), path.clone()));

        let id = session.read_with(cx, |session, _| session.stocktake().search("")[0]);
        session.update(cx, |session, cx| {
            session.record_count(id, "A1", 4, false, cx)
        });

        let saved = store::load(&path).unwrap().unwrap();
        assert_eq!(saved.product(id).counted_quantity(), Some(4));
        session.read_with(cx, |session, _| {
            assert_eq!(session.save_state(), SaveState::Saved)
        });

        session
            .read_with(cx, |session, _| session.discard())
            .unwrap();
        assert_eq!(store::load(&path).unwrap(), None);
        std::fs::remove_dir_all(dir).ok();
    }

    #[gpui_kit::test]
    fn a_failed_save_keeps_the_count_and_says_so(cx: &mut TestAppContext) {
        // A directory can't be written over with a file.
        let dir = std::env::temp_dir().join(format!("stocktake-blocked-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let session = cx.new(|_| Session::new(stocktake(), dir.clone()));

        let events = Rc::new(RefCell::new(Vec::new()));
        let _subscription = cx.update({
            let events = events.clone();
            |cx| {
                cx.subscribe(&session, move |_, event: &SessionEvent, _| {
                    events.borrow_mut().push(event.clone())
                })
            }
        });

        let id = session.read_with(cx, |session, _| session.stocktake().search("")[0]);
        session.update(cx, |session, cx| {
            session.record_count(id, "A1", 4, false, cx)
        });
        cx.run_until_parked();

        session.read_with(cx, |session, _| {
            assert_eq!(session.save_state(), SaveState::Failed);
            assert_eq!(session.stocktake().product(id).counted_quantity(), Some(4));
        });
        assert!(matches!(
            events.borrow().as_slice(),
            [SessionEvent::SaveFailed(_)]
        ));
        std::fs::remove_file(dir.with_extension("json.tmp")).ok();
        std::fs::remove_dir_all(dir).ok();
    }
}
