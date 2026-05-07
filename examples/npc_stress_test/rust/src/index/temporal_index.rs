use crate::domain::{Tick, TimeWindow, WindowId};

pub fn active_ids_at<'a>(
    ids: impl IntoIterator<Item = &'a WindowId>,
    windows: &[TimeWindow],
    tick: Tick,
) -> Vec<WindowId> {
    let mut out = Vec::new();
    for id in ids {
        let window = &windows[*id as usize];
        if window.start_tick > tick {
            break;
        }
        if window.active_at(tick) {
            out.push(*id);
        }
    }
    out
}

pub fn overlapping_ids<'a>(
    ids: impl IntoIterator<Item = &'a WindowId>,
    windows: &[TimeWindow],
    start_tick: Tick,
    end_tick: Tick,
) -> Vec<WindowId> {
    let mut out = Vec::new();
    for id in ids {
        let window = &windows[*id as usize];
        if window.start_tick >= end_tick {
            break;
        }
        if window.overlaps_range(start_tick, end_tick) {
            out.push(*id);
        }
    }
    out
}
