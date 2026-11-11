use crate::rates::Rates;
use crate::region::Tier;
use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::env;
use std::process::Command;

pub const BASE_URL: &str = "https://cloudbilling.googleapis.com/v2beta";
pub const CLOUD_RUN_SERVICE_NAME: &str = "services/152E-C115-5142";

/// Publicly listed Cloud Run SKUs for request-based billing in `cloudbilling:v2beta`.
pub const SKU_CPU_TIER1: &str = "4856-B847-F1EB"; // "Services CPU (Request-based billing)"
pub const SKU_CPU_TIER2: &str = "085C-A237-027A"; // "Services CPU Tier 2  (Request-based billing)"
pub const SKU_MEMORY_TIER1: &str = "02A2-9231-36A6"; // "Services Memory (Request-based billing)"
pub const SKU_MEMORY_TIER2: &str = "600C-3782-6708"; // "Services Memory Tier 2 (Request-based billing)"
pub const SKU_REQUESTS: &str = "2DA5-55D3-E679"; // "Requests"

#[derive(Debug, Clone)]
pub enum ApiAuth {
    BearerToken(String),
    ApiKey(String),
}

impl ApiAuth {
    /// Resolves credentials for the Cloud Billing v2beta API from environment variables
    /// or the local `gcloud` CLI.
    pub fn from_env_or_gcloud() -> Result<Self> {
        for var in ["CLOUD_BILLING_ACCESS_TOKEN", "GOOGLE_OAUTH_ACCESS_TOKEN"] {
            if let Ok(token) = env::var(var) {
                let trimmed = token.trim();
                if !trimmed.is_empty() {
                    return Ok(Self::BearerToken(trimmed.to_string()));
                }
            }
        }

        for var in ["CLOUD_BILLING_API_KEY", "GOOGLE_API_KEY"] {
            if let Ok(key) = env::var(var) {
                let trimmed = key.trim();
                if !trimmed.is_empty() {
                    return Ok(Self::ApiKey(trimmed.to_string()));
                }
            }
        }

        // Fall back to `gcloud auth print-access-token` or `gcloud auth application-default print-access-token`
        for args in [
            &["auth", "print-access-token"][..],
            &["auth", "application-default", "print-access-token"][..],
        ] {
            if let Ok(output) = Command::new("gcloud").args(args).output() {
                if output.status.success() {
                    let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !token.is_empty() {
                        return Ok(Self::BearerToken(token));
                    }
                }
            }
        }

        Err(anyhow!(
            "No Cloud Billing API credentials found. Set GOOGLE_OAUTH_ACCESS_TOKEN, \
             GOOGLE_API_KEY, or authenticate with `gcloud auth login`."
        ))
    }
}

/// Matches `GoogleCloudBillingPricesV2betaPrice` from the v2beta discovery document.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceResponse {
    pub name: String,
    pub currency_code: Option<String>,
    #[serde(default)]
    pub sku_prices: Vec<SkuPrice>,
}

/// Matches `GoogleCloudBillingPricesV2betaSkuPrice`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkuPrice {
    pub consumption_model: Option<String>,
    pub consumption_model_description: Option<String>,
    pub value_type: Option<String>,
    pub rate: Option<Rate>,
}

/// Matches `GoogleCloudBillingPricesV2betaRate`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rate {
    #[serde(default)]
    pub tiers: Vec<RateTier>,
    pub unit_info: Option<UnitInfo>,
}

/// Matches `GoogleCloudBillingPricesV2betaRateTier`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateTier {
    pub start_amount: Option<Decimal>,
    pub list_price: Option<Money>,
}

/// Matches `GoogleCloudBillingPricesV2betaUnitInfo`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnitInfo {
    pub unit: Option<String>,
    pub unit_description: Option<String>,
    pub unit_quantity: Option<Decimal>,
}

/// Matches `Decimal`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Decimal {
    pub value: Option<String>,
}

/// Matches `Money`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Money {
    pub currency_code: Option<String>,
    pub units: Option<String>,
    pub nanos: Option<i64>,
}

impl Money {
    pub fn to_f64(&self) -> Result<f64> {
        let units: f64 = self
            .units
            .as_deref()
            .unwrap_or("0")
            .parse()
            .context("Invalid Money.units")?;
        let nanos = self.nanos.unwrap_or(0) as f64 * 1e-9;
        Ok(units + nanos)
    }
}

/// Fetches the `PriceResponse` for a single SKU from `GET /v2beta/skus/{sku_id}/price`.
pub fn fetch_sku_price(sku_id: &str, auth: &ApiAuth) -> Result<PriceResponse> {
    let url = format!("{}/skus/{}/price", BASE_URL, sku_id);
    let req = match auth {
        ApiAuth::BearerToken(token) => {
            ureq::get(&url).set("Authorization", &format!("Bearer {}", token))
        }
        ApiAuth::ApiKey(key) => ureq::get(&url).query("key", key),
    };

    let response = req
        .call()
        .with_context(|| format!("Failed to fetch price for SKU {}", sku_id))?;

    let price: PriceResponse = response
        .into_json()
        .with_context(|| format!("Failed to deserialize price JSON for SKU {}", sku_id))?;
    Ok(price)
}

/// Extracts the non-zero "Default" consumption model price per base unit from a `PriceResponse`.
/// If `target_unit_quantity` is e.g. `1.0`, returns the price per 1 unit.
/// If `target_unit_quantity` is `1_000_000.0` (e.g. per million requests), scales accordingly.
pub fn extract_default_rate(price: &PriceResponse, target_unit_quantity: f64) -> Result<f64> {
    let default_sku_price = price
        .sku_prices
        .iter()
        .find(|sp| sp.consumption_model_description.as_deref() == Some("Default"))
        .ok_or_else(|| anyhow!("No 'Default' consumption model found in {}", price.name))?;

    let rate = default_sku_price
        .rate
        .as_ref()
        .ok_or_else(|| anyhow!("Missing rate object in {}", price.name))?;

    let unit_quantity: f64 = rate
        .unit_info
        .as_ref()
        .and_then(|u| u.unit_quantity.as_ref())
        .and_then(|d| d.value.as_deref())
        .unwrap_or("1")
        .parse()
        .context("Invalid unitQuantity")?;

    if unit_quantity <= 0.0 {
        return Err(anyhow!("Non-positive unitQuantity {} in {}", unit_quantity, price.name));
    }

    // Select the first non-zero listPrice tier (skipping any $0 free tier), or 0.0 if all are 0.
    let mut selected_price = None;
    for tier in &rate.tiers {
        if let Some(list_price) = &tier.list_price {
            let val = list_price.to_f64()?;
            if val > 0.0 {
                selected_price = Some(val);
                break;
            }
        }
    }

    let tier_price = selected_price.ok_or_else(|| {
        anyhow!("No non-zero rate tier found for 'Default' model in {}", price.name)
    })?;

    Ok((tier_price / unit_quantity) * target_unit_quantity)
}

/// Fetches live Cloud Run request-based `Rates` for a specific pricing tier from the v2beta API.
pub fn fetch_rates_for_tier(tier: Tier) -> Result<Rates> {
    let auth = ApiAuth::from_env_or_gcloud()?;
    fetch_rates_for_tier_with_auth(tier, &auth)
}

/// Fetches live Cloud Run request-based `Rates` for a specific pricing tier using explicit auth.
pub fn fetch_rates_for_tier_with_auth(tier: Tier, auth: &ApiAuth) -> Result<Rates> {
    let (cpu_sku, mem_sku) = match tier {
        Tier::Tier1 => (SKU_CPU_TIER1, SKU_MEMORY_TIER1),
        Tier::Tier2 => (SKU_CPU_TIER2, SKU_MEMORY_TIER2),
    };

    let cpu_price = fetch_sku_price(cpu_sku, auth)?;
    let mem_price = fetch_sku_price(mem_sku, auth)?;
    let req_price = fetch_sku_price(SKU_REQUESTS, auth)?;

    Ok(Rates {
        cpu_per_vcpu_second: extract_default_rate(&cpu_price, 1.0)?,
        memory_per_gib_second: extract_default_rate(&mem_price, 1.0)?,
        per_million_requests: extract_default_rate(&req_price, 1_000_000.0)?,
    })
}

/// Fetches both Tier 1 and Tier 2 `Rates` from the Cloud Billing v2beta API in a single session
/// (reusing the `Requests` SKU call across both tiers).
pub fn fetch_all_tier_rates() -> Result<(Rates, Rates)> {
    let auth = ApiAuth::from_env_or_gcloud()?;
    let cpu_t1 = extract_default_rate(&fetch_sku_price(SKU_CPU_TIER1, &auth)?, 1.0)?;
    let mem_t1 = extract_default_rate(&fetch_sku_price(SKU_MEMORY_TIER1, &auth)?, 1.0)?;
    let cpu_t2 = extract_default_rate(&fetch_sku_price(SKU_CPU_TIER2, &auth)?, 1.0)?;
    let mem_t2 = extract_default_rate(&fetch_sku_price(SKU_MEMORY_TIER2, &auth)?, 1.0)?;
    let req_rate = extract_default_rate(&fetch_sku_price(SKU_REQUESTS, &auth)?, 1_000_000.0)?;

    let tier1 = Rates {
        cpu_per_vcpu_second: cpu_t1,
        memory_per_gib_second: mem_t1,
        per_million_requests: req_rate,
    };
    let tier2 = Rates {
        cpu_per_vcpu_second: cpu_t2,
        memory_per_gib_second: mem_t2,
        per_million_requests: req_rate,
    };

    Ok((tier1, tier2))
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAX_ALLOWED_DIFF_RATIO: f64 = 0.10; // 10%

    fn relative_diff(hardcoded: f64, api: f64) -> f64 {
        if api == 0.0 {
            (hardcoded - api).abs()
        } else {
            (hardcoded - api).abs() / api.abs()
        }
    }

    fn assert_within_10_percent(label: &str, hardcoded: f64, api: f64) {
        let diff = relative_diff(hardcoded, api);
        assert!(
            diff <= MAX_ALLOWED_DIFF_RATIO,
            "{} differs by {:.2}% (hardcoded: {}, api: {}), which exceeds the 10% threshold",
            label,
            diff * 100.0,
            hardcoded,
            api
        );
    }

    #[test]
    fn test_extract_default_rate_from_sample_json() {
        let sample_json = r#"{
            "name": "skus/2DA5-55D3-E679/price",
            "currencyCode": "USD",
            "skuPrices": [
                {
                    "consumptionModel": "consumptionModels/7754-699E-0EBF",
                    "consumptionModelDescription": "Default",
                    "valueType": "rate",
                    "rate": {
                        "tiers": [
                            {
                                "startAmount": { "value": "0" },
                                "listPrice": { "currencyCode": "USD" }
                            },
                            {
                                "startAmount": { "value": "2000000" },
                                "listPrice": { "currencyCode": "USD", "nanos": 400000000 }
                            }
                        ],
                        "unitInfo": {
                            "unit": "count",
                            "unitDescription": "count",
                            "unitQuantity": { "value": "1000000" }
                        }
                    }
                }
            ]
        }"#;

        let parsed: PriceResponse = serde_json::from_str(sample_json).unwrap();
        let rate = extract_default_rate(&parsed, 1_000_000.0).unwrap();
        assert!((rate - 0.40).abs() < 1e-9);
    }

    #[test]
    fn test_api_rates_within_10_percent_of_hardcoded() {
        let (api_t1, api_t2) = fetch_all_tier_rates()
            .expect("Failed to fetch Cloud Run rates from Cloud Billing v2beta API");

        let hc_t1 = Rates::for_tier(Tier::Tier1);
        let hc_t2 = Rates::for_tier(Tier::Tier2);

        // Compare Tier 1 individual rates
        assert_within_10_percent(
            "Tier 1 CPU (per vCPU-second)",
            hc_t1.cpu_per_vcpu_second,
            api_t1.cpu_per_vcpu_second,
        );
        assert_within_10_percent(
            "Tier 1 Memory (per GiB-second)",
            hc_t1.memory_per_gib_second,
            api_t1.memory_per_gib_second,
        );
        assert_within_10_percent(
            "Tier 1 Requests (per million)",
            hc_t1.per_million_requests,
            api_t1.per_million_requests,
        );

        // Compare Tier 2 individual rates
        assert_within_10_percent(
            "Tier 2 CPU (per vCPU-second)",
            hc_t2.cpu_per_vcpu_second,
            api_t2.cpu_per_vcpu_second,
        );
        assert_within_10_percent(
            "Tier 2 Memory (per GiB-second)",
            hc_t2.memory_per_gib_second,
            api_t2.memory_per_gib_second,
        );
        assert_within_10_percent(
            "Tier 2 Requests (per million)",
            hc_t2.per_million_requests,
            api_t2.per_million_requests,
        );
    }
}
