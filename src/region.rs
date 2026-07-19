use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    Tier1,
    Tier2,
}

impl fmt::Display for Tier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Tier1 => write!(f, "Tier 1"),
            Self::Tier2 => write!(f, "Tier 2"),
        }
    }
}

pub const TIER1: &[&str] = &[
    "africa-south1",
    "asia-east1",
    "asia-northeast1",
    "asia-northeast2",
    "asia-south1",
    "asia-southeast3",
    "europe-north1",
    "europe-north2",
    "europe-southwest1",
    "europe-west1",
    "europe-west4",
    "europe-west8",
    "europe-west9",
    "me-west1",
    "northamerica-south1",
    "us-central1",
    "us-east1",
    "us-east4",
    "us-east5",
    "us-south1",
    "us-west1",
    "us-west8",
];

pub const TIER2: &[&str] = &[
    "asia-east2",
    "asia-northeast3",
    "asia-southeast1",
    "asia-southeast2",
    "asia-south2",
    "australia-southeast1",
    "australia-southeast2",
    "europe-central2",
    "europe-west10",
    "europe-west12",
    "europe-west2",
    "europe-west3",
    "europe-west6",
    "me-central1",
    "me-central2",
    "northamerica-northeast1",
    "northamerica-northeast2",
    "southamerica-east1",
    "southamerica-west1",
    "us-west2",
    "us-west3",
    "us-west4",
];

pub fn known(name: &str) -> bool {
    TIER1.contains(&name) || TIER2.contains(&name)
}

pub fn tier(name: &str) -> Option<Tier> {
    if TIER1.contains(&name) {
        Some(Tier::Tier1)
    } else if TIER2.contains(&name) {
        Some(Tier::Tier2)
    } else {
        None
    }
}

pub fn all() -> Vec<String> {
    let mut us = Vec::new();
    let mut rest = Vec::new();

    for &name in TIER1.iter().chain(TIER2.iter()) {
        if name.starts_with("us-") {
            us.push(name.to_string());
        } else {
            rest.push(name.to_string());
        }
    }

    us.sort();
    rest.sort();
    us.extend(rest);
    us
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier() {
        assert_eq!(tier("us-central1"), Some(Tier::Tier1));
        assert_eq!(tier("us-west2"), Some(Tier::Tier2));
        assert_eq!(tier("mars-north1"), None);
    }

    #[test]
    fn test_known() {
        assert!(known("europe-west1"));
        assert!(!known("nowhere"));
    }
}
