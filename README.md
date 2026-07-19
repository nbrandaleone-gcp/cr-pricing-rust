# cr-pricing-rust

A Rust-based implementation of the Google Cloud Run pricing calculator. Estimates the monthly cost of a request-based (pay-per-use) Cloud Run service from CPU, memory, request volume, average request duration, and region.

It offers two interfaces:
- **Interactive TUI (Default)**: A terminal dashboard featuring interactive parameter sliders with live-updating monthly cost reports.
- **Non-Interactive CLI**: A scriptable command-line interface utilizing flag options for pipeline and scripting automation.

---

## How It Works

Cloud Run request-based billing bills CPU and memory allocations *only* while an instance is actively processing requests (plus startup/shutdown). The calculation logic models the default request-based pay-per-use configuration (ignoring free tiers, idle minimum instances, and committed-use discounts).

For each resource, the monthly cost is calculated as follows:
1. **Billable Instance Time**:
   $$\text{Billable Seconds} = \frac{\text{Requests} \times \lceil\text{Average Request Duration (ms)} \to 100\text{ms}\rceil}{\text{Concurrency}}$$
   *Active request duration is rounded up to the nearest 100ms granularity. Concurrent requests share a single instance's billed time.*
2. **CPU Cost**:
   $$\text{CPU Cost} = \text{Billable Seconds} \times \text{vCPUs} \times \text{CPU Rate}$$
3. **Memory Cost**:
   $$\text{Memory Cost} = \text{Billable Seconds} \times \left(\frac{\text{Memory MiB}}{1024}\right) \times \text{Memory Rate}$$
4. **Request Cost**:
   $$\text{Request Cost} = \left(\frac{\text{Requests}}{1,000,000}\right) \times \text{Request Rate}$$

### Price Tiers (USD)
Rates depend on the region's pricing tier (Tier 1 vs. Tier 2):

| Resource | Tier 1 | Tier 2 |
| :--- | :--- | :--- |
| **CPU** (per vCPU-second) | `$0.000024` | `$0.00003360` |
| **Memory** (per GiB-second) | `$0.0000025` | `$0.00000350` |
| **Requests** (per million) | `$0.40` | `$0.40` |

---

## Build & Test Instructions

### Prerequisites
Make sure you have Rust and Cargo installed:
* [Install Rust](https://www.rust-lang.org/tools/install)

### Building the Project
Compile the release binary:
```bash
cargo build --release
```
The compiled executable will be located at `target/release/cr-pricing`.

### Running Unit Tests
Run the unit test suite:
```bash
cargo test
```

---

## Usage & Verification

### 1. Interactive TUI Mode
Run the executable with **no arguments** to launch the interactive terminal interface:
```bash
cargo run --release
# or
./target/release/cr-pricing
```

#### Controls & Navigation
Use the keyboard to navigate and adjust parameters:

| Key | Action |
| :--- | :--- |
| `↑` / `↓` (or `k` / `j`) | Select slider row |
| `←` / `→` (or `h` / `l`) | Decrease / Increase value by step |
| `PageUp` / `PageDown` | Jump value by large step (Memory: 4096 MiB, Requests: 10M) |
| `q` / `Esc` | Quit the application safely |

---

### 2. Command-Line (CLI) Mode
Pass flags to run calculations non-interactively.

```bash
./target/release/cr-pricing [options]
```

#### Available Flags
| Flag | Description | Default |
| :--- | :--- | :--- |
| `-c`, `--cpu` | vCPU allocated per instance | `1.0` |
| `-m`, `--memory` | Memory in MiB per instance | `512` |
| `-r`, `--requests` | HTTP requests per month | `2,000,000` |
| `-R`, `--region` | Cloud Run region | `us-central1` |
| `-d`, `--duration` | Average request duration in milliseconds | `100.0` |
| `-n`, `--concurrency` | Concurrent requests per instance | `1` |
| `--list-regions` | List supported regions and pricing tiers | — |
| `-h`, `--help` | Show help usage output | — |

#### Verification Examples

* **Default Calculation**:
  ```bash
  ./target/release/cr-pricing --cpu 1 --memory 512 --requests 2000000 --region us-central1
  ```
  *Output:*
  ```text
  Google Cloud Run pricing estimate (request-based billing, free tier ignored)
  ==========================================================================

  Configuration
    Region:        us-central1 (Tier 1)
    CPU:           1 vCPU
    Memory:        512 MiB
    Requests:      2,000,000 / month
    Avg duration:  100 ms
    Concurrency:   1

  Monthly cost breakdown
    CPU:           $4.80
    Memory:        $0.25
    Requests:      $0.80
    ------------------------------
    Total:         $5.85
  ```

* **High Performance / Tier 2 Region**:
  ```bash
  ./target/release/cr-pricing --cpu 2 --memory 4096 --requests 10000000 --region europe-west3 --duration 250 --concurrency 4
  ```

* **Listing Regions**:
  ```bash
  ./target/release/cr-pricing --list-regions
  ```

---

## Docker Packaging

Build the container image using the multi-stage [Dockerfile](file:///Users/nbrand/projects/cr-pricing-rust/Dockerfile):
```bash
docker build -t cr-pricing-rust .
```

Run the containerized application:
* For CLI mode:
  ```bash
  docker run --rm cr-pricing-rust --cpu 2 --memory 1024
  ```
* For TUI mode (requires attaching terminal interactively):
  ```bash
  docker run --rm -it cr-pricing-rust
  ```
