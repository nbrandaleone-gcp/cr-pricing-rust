use crate::calculator::{self, Inputs};
use crate::region;
use clap::Parser;

pub const DEFAULT_CPU: f64 = 1.0;
pub const DEFAULT_MEMORY_MIB: f64 = 512.0;
pub const DEFAULT_REQUESTS: u64 = 2_000_000;
pub const DEFAULT_REGION: &str = "us-central1";
pub const DEFAULT_REQUEST_MS: f64 = 100.0;
pub const DEFAULT_CONCURRENCY: u32 = 1;

#[derive(Parser, Debug)]
#[command(
    name = "cr-pricing",
    about = "Estimate the monthly cost of a request-based Cloud Run service.",
    long_about = "Estimate the monthly cost of a request-based Cloud Run service.\n\nUsage: cr-pricing [options]",
    disable_help_flag = true,
)]
pub struct CliArgs {
    #[arg(short = 'c', long = "cpu", default_value_t = DEFAULT_CPU, help = "vCPU allocated per instance")]
    pub cpu: f64,

    #[arg(short = 'm', long = "memory", default_value_t = DEFAULT_MEMORY_MIB, help = "Memory in MiB per instance")]
    pub memory: f64,

    #[arg(short = 'r', long = "requests", default_value_t = DEFAULT_REQUESTS, help = "HTTP requests per month")]
    pub requests: u64,

    #[arg(short = 'R', long = "region", default_value = DEFAULT_REGION, help = "Cloud Run region")]
    pub region: String,

    #[arg(short = 'd', long = "duration", default_value_t = DEFAULT_REQUEST_MS, help = "Average request duration in ms")]
    pub duration: f64,

    #[arg(short = 'n', long = "concurrency", default_value_t = DEFAULT_CONCURRENCY, help = "Concurrent requests per instance")]
    pub concurrency: u32,

    #[arg(long = "list-regions", help = "List supported regions and their pricing tier")]
    pub list_regions: bool,

    #[arg(short = 'h', long = "help", help = "Show this help")]
    pub help: bool,
}

pub fn run(args: CliArgs) -> i32 {
    if args.help {
        print_help();
        return 0;
    }

    if args.list_regions {
        print_regions();
        return 0;
    }

    // Validate inputs
    if args.cpu <= 0.0 {
        eprintln!("Value for --cpu must be greater than 0");
        return 1;
    }
    if args.memory <= 0.0 {
        eprintln!("Value for --memory must be greater than 0");
        return 1;
    }
    if args.requests == 0 {
        eprintln!("Value for --requests must be greater than 0");
        return 1;
    }
    if args.duration <= 0.0 {
        eprintln!("Value for --duration must be greater than 0");
        return 1;
    }
    if args.concurrency == 0 {
        eprintln!("Value for --concurrency must be greater than 0");
        return 1;
    }

    if !region::known(&args.region) {
        eprintln!(
            "Error: unknown region '{}'. Use --list-regions to see supported regions.",
            args.region
        );
        return 1;
    }

    let inputs = Inputs {
        cpu: args.cpu,
        memory_mib: args.memory,
        requests: args.requests,
        region: args.region,
        avg_request_ms: args.duration,
        concurrency: args.concurrency,
    };

    match calculator::estimate(&inputs) {
        Ok(est) => {
            report(&inputs, &est);
            0
        }
        Err(e) => {
            eprintln!("Error calculating estimate: {}", e);
            1
        }
    }
}

fn print_help() {
    println!(
        "Estimate the monthly cost of a request-based Cloud Run service.\n\n\
        Usage: cr-pricing [options]\n\
        \n\
        Options:\n\
        \x20\x20-c CPU, --cpu=CPU                vCPU allocated per instance (default: {})\n\
        \x20\x20-m MIB, --memory=MIB             Memory in MiB per instance (default: {})\n\
        \x20\x20-r N, --requests=N               HTTP requests per month (default: {})\n\
        \x20\x20-R REGION, --region=REGION       Cloud Run region (default: {})\n\
        \x20\x20-d MS, --duration=MS             Average request duration in ms (default: {})\n\
        \x20\x20-n N, --concurrency=N            Concurrent requests per instance (default: {})\n\
        \x20\x20--list-regions                   List supported regions and their pricing tier\n\
        \x20\x20-h, --help                       Show this help",
        DEFAULT_CPU,
        DEFAULT_MEMORY_MIB as u64,
        DEFAULT_REQUESTS,
        DEFAULT_REGION,
        DEFAULT_REQUEST_MS as u64,
        DEFAULT_CONCURRENCY
    );
}

fn print_regions() {
    println!("Supported Cloud Run regions:");
    for name in region::all() {
        let tier_str = match region::tier(&name) {
            Some(t) => t.to_string(),
            None => "".to_string(),
        };
        println!("  {:24} {}", name, tier_str);
    }
}

fn report(inputs: &Inputs, est: &calculator::Estimate) {
    println!("Google Cloud Run pricing estimate (request-based billing, free tier ignored)");
    println!("{}", "=".repeat(74));
    println!();
    println!("Configuration");
    println!("  Region:        {} ({})", inputs.region, est.tier);
    println!("  CPU:           {} vCPU", format_number(inputs.cpu));
    println!("  Memory:        {} MiB", format_number(inputs.memory_mib));
    println!("  Requests:      {} / month", format_int(inputs.requests));
    println!("  Avg duration:  {} ms", format_number(inputs.avg_request_ms));
    println!("  Concurrency:   {}", inputs.concurrency);
    println!();
    println!("Monthly cost breakdown");
    println!("  CPU:           {}", format_money(est.cpu_cost));
    println!("  Memory:        {}", format_money(est.memory_cost));
    println!("  Requests:      {}", format_money(est.request_cost));
    println!("  {}", "-".repeat(30));
    println!("  Total:         {}", format_money(est.total()));
}

fn format_money(amount: f64) -> String {
    format!("${:.2}", amount)
}

fn format_number(value: f64) -> String {
    if value == value.trunc() {
        format!("{}", value as u64)
    } else {
        format!("{}", value)
    }
}

fn format_int(value: u64) -> String {
    let s = value.to_string();
    let mut result = String::new();
    let bytes = s.as_bytes();
    let len = bytes.len();
    for (i, &byte) in bytes.iter().enumerate() {
        result.push(byte as char);
        let rem = len - 1 - i;
        if rem > 0 && rem % 3 == 0 {
            result.push(',');
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_int() {
        assert_eq!(format_int(2000000), "2,000,000");
        assert_eq!(format_int(123), "123");
        assert_eq!(format_int(12345678), "12,345,678");
    }

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(512.0), "512");
        assert_eq!(format_number(1.5), "1.5");
        assert_eq!(format_number(0.0), "0");
    }

    #[test]
    fn test_format_money() {
        assert_eq!(format_money(4.80), "$4.80");
        assert_eq!(format_money(0.25), "$0.25");
        assert_eq!(format_money(12.3456), "$12.35");
    }
}
