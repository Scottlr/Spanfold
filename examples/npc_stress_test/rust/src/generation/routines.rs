use std::collections::BTreeMap;

use crate::{
    domain::{ActivityKind, Archetype, Person, PersonId, Tick, TimeWindow},
    generation::{geography, relationships},
};

#[derive(Clone, Debug)]
pub struct GenerationConfig {
    pub people: u32,
    pub seed: u64,
}

impl Default for GenerationConfig {
    fn default() -> Self {
        Self {
            people: 10_000,
            seed: 42,
        }
    }
}

#[derive(Clone, Debug)]
pub struct WorldData {
    pub seed: u64,
    pub people: Vec<Person>,
    pub windows: Vec<TimeWindow>,
    pub connections: Vec<crate::domain::PersonConnection>,
}

#[derive(Clone, Debug)]
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    fn next_u32(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 32) as u32
    }

    fn range(&mut self, min: u32, max: u32) -> u32 {
        min + self.next_u32() % (max - min + 1)
    }
}

pub fn generate_world(config: GenerationConfig) -> Result<WorldData, String> {
    let mut people = Vec::with_capacity(config.people as usize);
    let mut windows = Vec::with_capacity(config.people as usize * 12);
    let mut next_window_id = 0_u32;

    for id in 0..config.people {
        let mut rng = Rng::new(config.seed ^ id as u64);
        let archetype = archetype_for(id, &mut rng);
        let home_index = (id.wrapping_mul(17) + rng.range(0, 63)) % geography::DISTRICT_COUNT;
        let work_index = work_index_for(id, archetype, &mut rng);
        let person = Person {
            id: PersonId(id),
            archetype,
            home_district: geography::district(home_index),
            work_district: geography::district(work_index),
            home_chunk: geography::chunk(home_index * 3 + id % 11),
        };
        let before = windows.len();
        generate_person_windows(
            &person,
            home_index,
            work_index,
            &mut rng,
            &mut next_window_id,
            &mut windows,
        )?;
        validate_person_windows(&windows[before..])?;
        people.push(person);
    }

    let connections = relationships::generate_connections(&people, config.seed);
    Ok(WorldData {
        seed: config.seed,
        people,
        windows,
        connections,
    })
}

fn archetype_for(id: u32, rng: &mut Rng) -> Archetype {
    if id == 12_345 || id.is_multiple_of(9_973) {
        return Archetype::SurveillanceTarget;
    }
    match (id + rng.range(0, 99)) % 100 {
        0..=32 => Archetype::OfficeWorker,
        33..=45 => Archetype::ShopWorker,
        46..=58 => Archetype::Student,
        59..=68 => Archetype::Retired,
        69..=76 => Archetype::NightShiftWorker,
        77..=86 => Archetype::Courier,
        87..=93 => Archetype::EmergencyWorker,
        _ => Archetype::CriminalActor,
    }
}

fn work_index_for(id: u32, archetype: Archetype, rng: &mut Rng) -> u32 {
    match archetype {
        Archetype::Retired => (id * 17 + 3) % geography::DISTRICT_COUNT,
        Archetype::Courier | Archetype::EmergencyWorker => {
            (id * 5 + rng.range(0, 63)) % geography::DISTRICT_COUNT
        }
        Archetype::CriminalActor => 23,
        Archetype::SurveillanceTarget => 23,
        _ => (id * 11 + 13 + rng.range(0, 31)) % geography::DISTRICT_COUNT,
    }
}

fn generate_person_windows(
    person: &Person,
    home_index: u32,
    work_index: u32,
    rng: &mut Rng,
    next_window_id: &mut u32,
    windows: &mut Vec<TimeWindow>,
) -> Result<(), String> {
    let jitter = rng.range(0, 1_800);
    let schedule: Vec<(Tick, Tick, ActivityKind, u32, &'static str)> = match person.archetype {
        Archetype::NightShiftWorker => vec![
            (
                0,
                7 * 3600 + jitter,
                ActivityKind::Working,
                work_index,
                "work",
            ),
            (
                7 * 3600 + jitter,
                8 * 3600 + jitter,
                ActivityKind::Commuting,
                work_index,
                "commute",
            ),
            (
                8 * 3600 + jitter,
                15 * 3600 + jitter,
                ActivityKind::Sleeping,
                home_index,
                "home",
            ),
            (
                15 * 3600 + jitter,
                18 * 3600,
                ActivityKind::Eating,
                home_index,
                "home",
            ),
            (
                18 * 3600,
                19 * 3600,
                ActivityKind::Commuting,
                home_index,
                "commute",
            ),
            (19 * 3600, 86_400, ActivityKind::Working, work_index, "work"),
        ],
        Archetype::Courier => courier_schedule(home_index, work_index, jitter),
        Archetype::Retired => vec![
            (
                0,
                7 * 3600 + jitter,
                ActivityKind::Sleeping,
                home_index,
                "home",
            ),
            (
                7 * 3600 + jitter,
                9 * 3600,
                ActivityKind::Eating,
                home_index,
                "home",
            ),
            (
                9 * 3600,
                11 * 3600,
                ActivityKind::Shopping,
                home_index,
                "shop",
            ),
            (
                11 * 3600,
                14 * 3600,
                ActivityKind::Socialising,
                (home_index + 1) % 64,
                "park",
            ),
            (
                14 * 3600,
                17 * 3600,
                ActivityKind::Loitering,
                home_index,
                "plaza",
            ),
            (
                17 * 3600,
                21 * 3600,
                ActivityKind::Eating,
                home_index,
                "home",
            ),
            (
                21 * 3600,
                86_400,
                ActivityKind::Sleeping,
                home_index,
                "home",
            ),
        ],
        Archetype::CriminalActor => criminal_schedule(home_index, work_index, jitter),
        Archetype::SurveillanceTarget => surveillance_schedule(home_index, work_index, jitter),
        _ => daytime_schedule(person.archetype, home_index, work_index, jitter),
    };

    for (start, end, activity, district, place) in schedule {
        if start >= end || start >= 86_400 {
            continue;
        }
        let end = end.min(86_400);
        let location = if place == "commute" {
            geography::commute_location(home_index, work_index, person.id.0, start)
        } else {
            geography::location_for(district, place, person.id.0, start)
        };
        let mut window =
            TimeWindow::new(*next_window_id, person.id, start, end, activity, location)?;
        *next_window_id += 1;
        window.tags.push(format!("{:?}", person.archetype));
        window.metadata = BTreeMap::from([
            ("archetype".to_owned(), format!("{:?}", person.archetype)),
            ("routine_seeded".to_owned(), "true".to_owned()),
        ]);
        windows.push(window);
    }
    Ok(())
}

fn daytime_schedule(
    archetype: Archetype,
    home: u32,
    work: u32,
    jitter: u32,
) -> Vec<(Tick, Tick, ActivityKind, u32, &'static str)> {
    let work_activity = ActivityKind::Working;
    vec![
        (0, 6 * 3600 + jitter, ActivityKind::Sleeping, home, "home"),
        (
            6 * 3600 + jitter,
            7 * 3600 + jitter,
            ActivityKind::Eating,
            home,
            "home",
        ),
        (
            7 * 3600 + jitter,
            8 * 3600 + jitter,
            ActivityKind::Commuting,
            home,
            "commute",
        ),
        (
            8 * 3600 + jitter,
            12 * 3600,
            work_activity,
            work,
            if archetype == Archetype::Student {
                "school"
            } else {
                "work"
            },
        ),
        (12 * 3600, 13 * 3600, ActivityKind::Eating, work, "lunch"),
        (
            13 * 3600,
            17 * 3600 + jitter / 2,
            work_activity,
            work,
            "work",
        ),
        (
            17 * 3600 + jitter / 2,
            18 * 3600 + jitter / 2,
            ActivityKind::Commuting,
            work,
            "commute",
        ),
        (
            18 * 3600 + jitter / 2,
            21 * 3600,
            ActivityKind::Socialising,
            (home + 2) % 64,
            "venue",
        ),
        (21 * 3600, 22 * 3600, ActivityKind::Eating, home, "home"),
        (22 * 3600, 86_400, ActivityKind::Sleeping, home, "home"),
    ]
}

fn courier_schedule(
    home: u32,
    work: u32,
    jitter: u32,
) -> Vec<(Tick, Tick, ActivityKind, u32, &'static str)> {
    let mut out = vec![(0, 6 * 3600 + jitter, ActivityKind::Sleeping, home, "home")];
    let mut tick = 6 * 3600 + jitter;
    for stop in 0..10 {
        let next = tick + 3_600;
        out.push((
            tick,
            next,
            ActivityKind::Travelling,
            (work + stop) % 64,
            "road",
        ));
        tick = next;
    }
    out.push((tick, 21 * 3600, ActivityKind::Eating, home, "home"));
    out.push((21 * 3600, 86_400, ActivityKind::Sleeping, home, "home"));
    out
}

fn criminal_schedule(
    home: u32,
    work: u32,
    jitter: u32,
) -> Vec<(Tick, Tick, ActivityKind, u32, &'static str)> {
    vec![
        (0, 10 * 3600 + jitter, ActivityKind::Sleeping, home, "home"),
        (
            10 * 3600 + jitter,
            14 * 3600,
            ActivityKind::Loitering,
            home,
            "plaza",
        ),
        (
            14 * 3600,
            18 * 3600,
            ActivityKind::Shopping,
            (home + 7) % 64,
            "shop",
        ),
        (18 * 3600, 21 * 3600, ActivityKind::Socialising, 23, "venue"),
        (
            21 * 3600,
            23 * 3600,
            ActivityKind::CriminalActivity,
            work,
            "safehouse",
        ),
        (23 * 3600, 86_400, ActivityKind::Travelling, home, "road"),
    ]
}

fn surveillance_schedule(
    home: u32,
    work: u32,
    jitter: u32,
) -> Vec<(Tick, Tick, ActivityKind, u32, &'static str)> {
    let mut schedule = daytime_schedule(Archetype::OfficeWorker, home, work, jitter);
    schedule.push((
        19 * 3600,
        20 * 3600,
        ActivityKind::SurveillanceTarget,
        23,
        "civic_center",
    ));
    schedule.sort_by_key(|entry| entry.0);
    // Keep the explicit target marker by trimming overlapping evening social window.
    schedule
        .into_iter()
        .filter(|(_, end, _, _, _)| *end <= 19 * 3600 || *end > 20 * 3600)
        .collect()
}

fn validate_person_windows(windows: &[TimeWindow]) -> Result<(), String> {
    for pair in windows.windows(2) {
        if pair[0].end_tick > pair[1].start_tick {
            return Err(format!(
                "person {:?} has overlapping windows {} and {}",
                pair[0].person_id, pair[0].window_id, pair[1].window_id
            ));
        }
    }
    Ok(())
}
