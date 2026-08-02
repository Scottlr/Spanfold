use crate::{ComparisonFinality, TemporalAxis};

use super::{EpisodeFragment, EpisodeId};

pub(super) fn create(
    window_name: &str,
    key: &str,
    source: Option<&str>,
    partition: Option<&str>,
    axis: TemporalAxis,
    fragments: &[EpisodeFragment],
    finality: &ComparisonFinality,
) -> EpisodeId {
    let mut hash = 0xcbf29ce484222325_u64;
    let mut write = |value: &str| {
        for byte in value
            .len()
            .to_string()
            .bytes()
            .chain([b':'])
            .chain(value.bytes())
        {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    };
    write("spanfold-rust-episode-v1");
    write(window_name);
    write(key);
    write(source.unwrap_or(""));
    write(partition.unwrap_or(""));
    write(match axis {
        TemporalAxis::ProcessingPosition => "processing-position",
        TemporalAxis::Timestamp => "timestamp",
    });
    for fragment in fragments {
        write(fragment.record_id());
        write(&fragment.range().start().magnitude().to_string());
        write(&fragment.range().end().magnitude().to_string());
        write(fragment.range().start().clock().unwrap_or(""));
        write(match fragment.finality() {
            ComparisonFinality::Final => "final",
            ComparisonFinality::Provisional => "provisional",
            ComparisonFinality::Revised => "revised",
            ComparisonFinality::Retracted => "retracted",
        });
    }
    write(match finality {
        ComparisonFinality::Final => "final",
        ComparisonFinality::Provisional => "provisional",
        ComparisonFinality::Revised => "revised",
        ComparisonFinality::Retracted => "retracted",
    });
    EpisodeId::new(format!("episode:{hash:016x}"))
}
