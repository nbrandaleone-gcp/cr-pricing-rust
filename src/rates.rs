use crate::pricing_api;
use crate::region::Tier;
use anyhow::Result;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rates {
    pub cpu_per_vcpu_second: f64,
    pub memory_per_gib_second: f64,
    pub per_million_requests: f64,
}

impl Rates {
    pub fn for_tier(tier: Tier) -> Self {
        match tier {
            Tier::Tier1 => Self {
                cpu_per_vcpu_second: 0.000024,
                memory_per_gib_second: 0.0000025,
                per_million_requests: 0.40,
            },
            Tier::Tier2 => Self {
                cpu_per_vcpu_second: 0.00003360,
                memory_per_gib_second: 0.00000350,
                per_million_requests: 0.40,
            },
        }
    }

    /// Fetches the latest rates for the given tier from the Cloud Billing v2beta API.
    pub fn from_api(tier: Tier) -> Result<Self> {
        pricing_api::fetch_rates_for_tier(tier)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rates_for_tier() {
        let tier1 = Rates::for_tier(Tier::Tier1);
        assert_eq!(tier1.cpu_per_vcpu_second, 0.000024);
        assert_eq!(tier1.memory_per_gib_second, 0.0000025);
        assert_eq!(tier1.per_million_requests, 0.40);

        let tier2 = Rates::for_tier(Tier::Tier2);
        assert_eq!(tier2.cpu_per_vcpu_second, 0.00003360);
        assert_eq!(tier2.memory_per_gib_second, 0.00000350);
        assert_eq!(tier2.per_million_requests, 0.40);
    }
}
