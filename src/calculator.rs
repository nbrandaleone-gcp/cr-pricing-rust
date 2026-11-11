use crate::rates::Rates;
use crate::region::{self, Tier};
use anyhow::{anyhow, Result};

pub const MIB_PER_GIB: f64 = 1024.0;
pub const BILLING_GRANULARITY_SECONDS: f64 = 0.1;

#[derive(Debug, Clone, PartialEq)]
pub struct Inputs {
    pub cpu: f64,
    pub memory_mib: f64,
    pub requests: u64,
    pub region: String,
    pub avg_request_ms: f64,
    pub concurrency: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Estimate {
    pub tier: Tier,
    pub rates: Rates,
    pub billable_seconds: f64,
    pub cpu_cost: f64,
    pub memory_cost: f64,
    pub request_cost: f64,
}

impl Estimate {
    pub fn total(&self) -> f64 {
        self.cpu_cost + self.memory_cost + self.request_cost
    }
}

pub fn estimate(inputs: &Inputs) -> Result<Estimate> {
    let tier = region::tier(&inputs.region)
        .ok_or_else(|| anyhow!("Unknown region: {}", inputs.region))?;
    let rates = Rates::for_tier(tier);
    estimate_with_rates(inputs, tier, rates)
}

pub fn estimate_from_api(inputs: &Inputs) -> Result<Estimate> {
    let tier = region::tier(&inputs.region)
        .ok_or_else(|| anyhow!("Unknown region: {}", inputs.region))?;
    let rates = Rates::from_api(tier)?;
    estimate_with_rates(inputs, tier, rates)
}

pub fn estimate_with_rates(inputs: &Inputs, tier: Tier, rates: Rates) -> Result<Estimate> {
    let billable_seconds = billable_seconds_for(inputs);
    let vcpu_seconds = billable_seconds * inputs.cpu;
    let gib_seconds = billable_seconds * (inputs.memory_mib / MIB_PER_GIB);

    let cpu_cost = vcpu_seconds * rates.cpu_per_vcpu_second;
    let memory_cost = gib_seconds * rates.memory_per_gib_second;
    let request_cost = (inputs.requests as f64 / 1_000_000.0) * rates.per_million_requests;

    Ok(Estimate {
        tier,
        rates,
        billable_seconds,
        cpu_cost,
        memory_cost,
        request_cost,
    })
}

fn billable_seconds_for(inputs: &Inputs) -> f64 {
    let per_request_seconds = round_up_to_granularity(inputs.avg_request_ms / 1000.0);
    (inputs.requests as f64 * per_request_seconds) / inputs.concurrency as f64
}

fn round_up_to_granularity(seconds: f64) -> f64 {
    (seconds / BILLING_GRANULARITY_SECONDS).ceil() * BILLING_GRANULARITY_SECONDS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_inputs() -> Inputs {
        Inputs {
            cpu: 1.0,
            memory_mib: 512.0,
            requests: 2_000_000,
            region: "us-central1".to_string(),
            avg_request_ms: 100.0,
            concurrency: 1,
        }
    }

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn test_estimate_default() {
        let inputs = default_inputs();
        let est = estimate(&inputs).unwrap();

        assert_eq!(est.billable_seconds, 200_000.0);
        assert!(approx_eq(est.cpu_cost, 4.80));
        assert!(approx_eq(est.memory_cost, 0.25));
        assert!(approx_eq(est.request_cost, 0.80));
        assert!(approx_eq(est.total(), 5.85));
    }

    #[test]
    fn test_rounds_up_duration() {
        let mut inputs = default_inputs();
        inputs.avg_request_ms = 150.0;
        let est = estimate(&inputs).unwrap();

        assert_eq!(est.billable_seconds, 400_000.0);
    }

    #[test]
    fn test_amortizes_compute_cost() {
        let inputs_single = default_inputs();
        let mut inputs_concurrent = default_inputs();
        inputs_concurrent.concurrency = 4;

        let single = estimate(&inputs_single).unwrap();
        let concurrent = estimate(&inputs_concurrent).unwrap();

        assert!(approx_eq(concurrent.cpu_cost, single.cpu_cost / 4.0));
        assert!(approx_eq(concurrent.memory_cost, single.memory_cost / 4.0));
        assert!(approx_eq(concurrent.request_cost, single.request_cost));
    }

    #[test]
    fn test_higher_rates_in_tier2() {
        let inputs_t1 = default_inputs();
        let mut inputs_t2 = default_inputs();
        inputs_t2.region = "us-west2".to_string();

        let t1 = estimate(&inputs_t1).unwrap();
        let t2 = estimate(&inputs_t2).unwrap();

        assert!(t2.cpu_cost > t1.cpu_cost);
        assert!(t2.memory_cost > t1.memory_cost);
    }

    #[test]
    fn test_scales_cost_with_cpu_and_mem() {
        let mut inputs = default_inputs();
        inputs.cpu = 2.0;
        inputs.memory_mib = 1024.0;

        let est = estimate(&inputs).unwrap();
        assert!(approx_eq(est.cpu_cost, 9.60));
        assert!(approx_eq(est.memory_cost, 0.50));
    }

    #[test]
    fn test_unknown_region() {
        let mut inputs = default_inputs();
        inputs.region = "atlantis1".to_string();

        let res = estimate(&inputs);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("Unknown region"));
    }

    #[test]
    fn test_compare_hardcoded_vs_api_estimates_within_10_percent() {
        let tier1_inputs = default_inputs();
        let mut tier2_inputs = default_inputs();
        tier2_inputs.region = "europe-west3".to_string();
        tier2_inputs.cpu = 2.0;
        tier2_inputs.memory_mib = 2048.0;
        tier2_inputs.requests = 10_000_000;
        tier2_inputs.avg_request_ms = 250.0;
        tier2_inputs.concurrency = 4;

        for inputs in [&tier1_inputs, &tier2_inputs] {
            let hc_est = estimate(inputs).unwrap();
            let api_est = estimate_from_api(inputs)
                .expect("Failed to calculate estimate from Cloud Billing v2beta API");

            for (label, hc_val, api_val) in [
                ("cpu_cost", hc_est.cpu_cost, api_est.cpu_cost),
                ("memory_cost", hc_est.memory_cost, api_est.memory_cost),
                ("request_cost", hc_est.request_cost, api_est.request_cost),
                ("total", hc_est.total(), api_est.total()),
            ] {
                let rel_diff = (hc_val - api_val).abs() / api_val.abs();
                assert!(
                    rel_diff <= 0.10,
                    "Region {} {} differs by {:.2}% (hardcoded: {}, api: {}), exceeding 10%",
                    inputs.region,
                    label,
                    rel_diff * 100.0,
                    hc_val,
                    api_val
                );
            }
        }
    }
}
