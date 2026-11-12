# Architecture Diagram

```mermaid
flowchart TD
    Main["main.rs :: main()"] -->|No CLI args| TUI["tui.rs :: run()"]
    Main -->|With CLI args| CLI["cli.rs :: CliArgs::parse() -> run()"]

    subgraph UI["User Interfaces"]
        TUI["tui.rs (ratatui + crossterm)<br/>SliderState, run_loop, draw"]
        CLI["cli.rs (clap)<br/>CliArgs, validation, report"]
    end

    subgraph Core["Core Pricing Logic"]
        Calc["calculator.rs<br/>Inputs, Estimate, estimate(),<br/>estimate_from_api(), estimate_with_rates()"]
        Region["region.rs<br/>Tier (Tier1 / Tier2),<br/>TIER1, TIER2, tier(), known(), all()"]
        Rates["rates.rs<br/>Rates, for_tier(), from_api()"]
    end

    subgraph External["Live Pricing Client"]
        API["pricing_api.rs (ureq + serde_json)<br/>ApiAuth::from_env_or_gcloud(),<br/>fetch_rates_for_tier(), fetch_sku_price(),<br/>extract_default_rate()"]
        GCP["Cloud Billing v2beta API<br/>cloudbilling.googleapis.com/v2beta/skus/{id}/price"]
        GCloud["Env Vars or gcloud CLI<br/>Access Token / API Key"]
    end

    TUI -->|region::all, region::tier| Region
    TUI -->|calculator::estimate| Calc

    CLI -->|--list-regions / region::known| Region
    CLI -->|default| Calc
    CLI -->|--use-api| Calc

    Calc -->|region::tier| Region
    Calc -->|Rates::for_tier / Rates::from_api| Rates

    Rates -->|from_api| API
    API -->|BearerToken / ApiKey| GCloud
    API -->|HTTPS GET per SKU| GCP
```

## Module Overview

- `src/main.rs`: Entry point that dispatches to the interactive TUI when invoked with no arguments, or parses flags and runs the CLI otherwise.
- `src/tui.rs`: Interactive terminal UI built with `ratatui` and `crossterm` for adjusting CPU, memory, monthly requests, execution duration, and region sliders.
- `src/cli.rs`: Non-interactive CLI built with `clap` supporting `--cpu`, `--memory`, `--requests`, `--region`, `--duration`, `--concurrency`, `--list-regions`, and `--use-api`.
- `src/calculator.rs`: Computes request-based monthly cost (`Estimate`) from `Inputs` using `0.1s` billing granularity rounding and `1024 MiB/GiB` conversion.
- `src/region.rs`: Maps Cloud Run regions to `Tier::Tier1` or `Tier::Tier2` and provides sorted region listing.
- `src/rates.rs`: Defines `Rates` with hardcoded tier rates (`Rates::for_tier`) and live lookup delegation (`Rates::from_api`).
- `src/pricing_api.rs`: Authenticates via environment variables or `gcloud` CLI and queries the Google Cloud Billing `v2beta` API for Cloud Run CPU, memory, and request SKUs.
