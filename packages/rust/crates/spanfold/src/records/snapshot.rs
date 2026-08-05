use std::cmp::Ordering;

use super::model::{WindowRecord, WindowSnapshotFinality, WindowSnapshotRecord};
use crate::{TemporalPoint, TemporalRange, TemporalRangeError};

pub(crate) fn snapshot_record(
    window: WindowRecord,
    horizon: TemporalPoint,
) -> Result<Option<WindowSnapshotRecord>, TemporalRangeError> {
    let start = window.start();
    if start.axis() != horizon.axis() || matches!(start.try_cmp(&horizon), Ok(Ordering::Greater)) {
        return Ok(None);
    }

    match window {
        WindowRecord::Closed(closed) => {
            if matches!(
                closed.range.end().try_cmp(&horizon),
                Ok(Ordering::Less | Ordering::Equal)
            ) {
                Ok(Some(WindowSnapshotRecord {
                    range: closed.range.clone(),
                    window: WindowRecord::Closed(closed),
                    finality: WindowSnapshotFinality::Final,
                }))
            } else {
                let range = TemporalRange::new(closed.range.start(), horizon)?;
                Ok(Some(WindowSnapshotRecord {
                    window: WindowRecord::Closed(closed),
                    range,
                    finality: WindowSnapshotFinality::Provisional,
                }))
            }
        }
        WindowRecord::Open(open) => {
            let range = TemporalRange::new(open.start.clone(), horizon.clone())?;
            Ok(Some(WindowSnapshotRecord {
                window: WindowRecord::Open(open),
                range,
                finality: WindowSnapshotFinality::Provisional,
            }))
        }
    }
}
