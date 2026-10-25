//! Argon2id calibration benchmark (M0 task 0.6) for the host machine.
//!
//! Prints a Markdown table of median derivation times over a grid of memory
//! and iteration costs. The iOS spike app runs the same grid on devices.
//!
//!   cargo run --release -p atlys-crypto --example argon2_bench [runs]

use std::time::Instant;

use atlys_crypto::kdf::{Argon2Params, derive_key_unchecked};

const MEMORY_MIB: [u32; 7] = [19, 46, 64, 128, 192, 256, 384];
const ITERATIONS: [u32; 5] = [1, 2, 3, 4, 6];

fn main() {
    let runs: usize = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(3);
    let salt = *b"atlys-bench-salt";
    let password = b"synthetic benchmark password";

    println!("Argon2id, p = 1, median of {runs} runs (ms)\n");
    print!("| m \\ t |");
    for t in ITERATIONS {
        print!(" t={t} |");
    }
    println!();
    println!("|---|{}", "---|".repeat(ITERATIONS.len()));
    for m in MEMORY_MIB {
        print!("| {m} MiB |");
        for t in ITERATIONS {
            let params = Argon2Params {
                memory_kib: m * 1024,
                iterations: t,
                parallelism: 1,
            };
            let mut times: Vec<f64> = (0..runs)
                .map(|_| {
                    let start = Instant::now();
                    let _key = derive_key_unchecked(password, &salt, params).expect("argon2");
                    start.elapsed().as_secs_f64() * 1000.0
                })
                .collect();
            times.sort_by(f64::total_cmp);
            print!(" {:.0} |", times[times.len() / 2]);
        }
        println!();
    }
}
