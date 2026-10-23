//! Memorable, local preset-name suggestions. No network or random crate required.
use super::{Book, library::Presets};
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

const FIRST: [&str; 32] = [
    "Velvet",
    "Quiet",
    "Electric",
    "Hidden",
    "Silver",
    "Golden",
    "Wild",
    "Soft",
    "Crystal",
    "Distant",
    "Gentle",
    "Radiant",
    "Amber",
    "Vivid",
    "Wandering",
    "Floating",
    "Scarlet",
    "Silent",
    "Lucid",
    "Neon",
    "Frozen",
    "Tender",
    "Endless",
    "Fleeting",
    "Burnished",
    "Midnight",
    "Opal",
    "Glowing",
    "Dreaming",
    "Weightless",
    "Violet",
    "Liquid",
];
const SECOND: [&str; 32] = [
    "Lunar", "Ocean", "Desert", "Forest", "Solar", "River", "Mountain", "Autumn", "Winter",
    "Summer", "Spring", "Cosmic", "Coral", "Prism", "Meadow", "Aurora", "Stellar", "Copper",
    "Indigo", "Canyon", "Cloud", "Cedar", "Granite", "Tidal", "Polar", "Orchid", "Saffron",
    "Emerald", "Glacier", "Willow", "Cobalt", "Horizon",
];
const THIRD: [&str; 32] = [
    "Drift",
    "Echo",
    "Bloom",
    "Current",
    "Pulse",
    "Orbit",
    "Cascade",
    "Ripple",
    "Shimmer",
    "Tide",
    "Spark",
    "Waltz",
    "Trace",
    "Flight",
    "Mirage",
    "Reverie",
    "Flicker",
    "Wave",
    "Glow",
    "Spiral",
    "Passage",
    "Rhapsody",
    "Arc",
    "Motion",
    "Whisper",
    "Reflection",
    "Resonance",
    "Vortex",
    "Daydream",
    "Ember",
    "Voyage",
    "Garden",
];

pub(super) fn suggest(book: &Book, presets: &Presets, previous: Option<&str>) -> String {
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    let mut seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
        ^ SERIAL
            .fetch_add(1, Ordering::Relaxed)
            .wrapping_mul(0x9e3779b97f4a7c15);
    seed = (seed ^ (seed >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    seed = (seed ^ (seed >> 27)).wrapping_mul(0x94d049bb133111eb);
    seed ^= seed >> 31;
    // A bounded walk guarantees a free name among 32^3 combinations, even if
    // random suggestions collide with existing names or the last canceled dialog.
    for offset in 0..32768 {
        let n = (seed as usize).wrapping_add(offset) % 32768;
        let name = format!(
            "{} {} {}",
            FIRST[n % 32],
            SECOND[n / 32 % 32],
            THIRD[n / 1024]
        );
        if previous != Some(name.as_str())
            && !book
                .scenes
                .iter()
                .chain(&presets.entries)
                .any(|s| s.name.eq_ignore_ascii_case(&name))
        {
            return name;
        }
    }
    unreachable!("scene and preset limits leave unused three-word combinations")
}
