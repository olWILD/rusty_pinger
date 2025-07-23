use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use surge_ping::{Client, Config, PingIdentifier, PingSequence};


#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Target host or IP
    target: Option<String>,

    #[arg(short, long, help = "Packets to send (default: continuous)")]
    count: Option<u64>,

    #[arg(short, long, default_value_t = 4.0, help = "Timeout per ping in seconds")]
    timeout: f64,

    #[arg(short = 's', long, default_value_t = 56, help = "ICMP payload size")]
    packet_size: usize,

    #[arg(short, long, default_value = "ping_history.json", help = "Output file")]
    output: String,

    #[arg(short = 'f', long, default_value = "json", help = "Output format: json or csv")]
    format: String,

    #[arg(short, long, help = "Output directory (default: current dir)")]
    directory: Option<PathBuf>,

    #[arg(long, help = "Interval in seconds to save results automatically")]
    save_interval: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct PingStats {
    target: String,
    timestamp: DateTime<Utc>,
    sent: u64,
    received: u64,
    loss_percent: f64,
    min: Option<f32>,
    max: Option<f32>,
    avg: Option<f32>,
    latency_buckets: HashMap<String, u64>,
}

impl PingStats {
    // Creates a new, empty stats object for a session.
    fn new(target: String) -> Self {
        let buckets = [
            "0-50ms", "50-100ms", "100-150ms", "150-200ms", "200-250ms",
            "250-300ms", "300-350ms", "350-400ms", "400-450ms", "450-500ms",
            "500-999ms", ">1000ms",
        ]
        .iter()
        .map(|s| (s.to_string(), 0))
        .collect();

        Self {
            target,
            timestamp: Utc::now(),
            sent: 0,
            received: 0,
            loss_percent: 0.0,
            min: None,
            max: None,
            avg: None,
            latency_buckets: buckets,
        }
    }

    // Recalculates all statistics based on the list of response times for the current session.
    fn calculate(&mut self, times: &[f32]) {
        self.received = times.len() as u64;
        self.timestamp = Utc::now();

        if self.sent > 0 {
            self.loss_percent = 100.0 * (self.sent - self.received) as f64 / self.sent as f64;
        }

        if times.is_empty() {
            self.min = None;
            self.max = None;
            self.avg = None;
        } else {
            self.min = Some((times.iter().fold(f32::MAX, |a, &b| a.min(b)) * 100.0).round() / 100.0);
            self.max = Some((times.iter().fold(f32::MIN, |a, &b| a.max(b)) * 100.0).round() / 100.0);
            self.avg = Some((times.iter().sum::<f32>() / times.len() as f32 * 100.0).round() / 100.0);
        }

        // Recalculate latency distribution buckets
        self.latency_buckets.values_mut().for_each(|v| *v = 0);
        for &time in times {
            let bucket = match time as u64 {
                0..=49 => "0-50ms",
                50..=99 => "50-100ms",
                100..=149 => "100-150ms",
                150..=199 => "150-200ms",
                200..=249 => "200-250ms",
                250..=299 => "250-300ms",
                300..=349 => "300-350ms",
                350..=399 => "350-400ms",
                400..=449 => "400-450ms",
                450..=499 => "450-500ms",
                500..=999 => "500-999ms",
                _ => ">1000ms",
            };
            *self.latency_buckets.entry(bucket.to_string()).or_insert(0) += 1;
        }
    }
}

// Saves the statistics by appending a new entry to the JSON file.
fn save_results(stats: &PingStats, path: &Path) -> Result<()> {
    // 1. Read all existing entries from the file.
    let mut entries: Vec<PingStats> = if path.exists() {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        serde_json::from_reader(reader).unwrap_or_else(|_| vec![])
    } else {
        vec![]
    };

    // 2. Simply add the new session's stats to the list.
    entries.push(stats.clone());

    // 3. Overwrite the file with the new, complete list.
    let file = OpenOptions::new().write(true).create(true).truncate(true).open(path)?;
    let writer = BufWriter::new(file);
    serde_json::to_writer_pretty(writer, &entries)?;
    Ok(())
}

// Saves the statistics by appending a new entry to the CSV file.
fn save_results_csv(stats: &PingStats, path: &Path) -> Result<()> {
    let file_exists = path.exists();
    let file = OpenOptions::new().write(true).create(true).append(true).open(path)?;
    let mut writer = csv::Writer::from_writer(file);

    // Write header if file is new
    if !file_exists {
        writer.write_record(&[
            "target", "timestamp", "sent", "received", "loss_percent", 
            "min", "max", "avg", "0-50ms", "50-100ms", "100-150ms", 
            "150-200ms", "200-250ms", "250-300ms", "300-350ms", 
            "350-400ms", "400-450ms", "450-500ms", "500-999ms", ">1000ms"
        ])?;
    }

    // Write data row
    let bucket_order = [
        "0-50ms", "50-100ms", "100-150ms", "150-200ms", "200-250ms",
        "250-300ms", "300-350ms", "350-400ms", "400-450ms", "450-500ms",
        "500-999ms", ">1000ms"
    ];
    
    let mut record = vec![
        stats.target.clone(),
        stats.timestamp.to_rfc3339(),
        stats.sent.to_string(),
        stats.received.to_string(),
        format!("{:.2}", stats.loss_percent),
        stats.min.map_or("".to_string(), |v| format!("{:.2}", v)),
        stats.max.map_or("".to_string(), |v| format!("{:.2}", v)),
        stats.avg.map_or("".to_string(), |v| format!("{:.2}", v)),
    ];
    
    for bucket in &bucket_order {
        record.push(stats.latency_buckets.get(*bucket).unwrap_or(&0).to_string());
    }
    
    writer.write_record(&record)?;
    writer.flush()?;
    Ok(())
}

// Generic save function that delegates to JSON or CSV based on file extension
fn save_results_generic(stats: &PingStats, path: &Path) -> Result<()> {
    if path.extension().and_then(|s| s.to_str()) == Some("csv") {
        save_results_csv(stats, path)
    } else {
        save_results(stats, path)
    }
}

fn print_current_results(stats: &PingStats) {
    println!("\n=== Current Session Stats ===");
    println!("Target: {}", stats.target);
    println!("Timestamp: {}", stats.timestamp);
    println!("Packets: Sent={}, Received={}", stats.sent, stats.received);
    println!("Packet Loss: {:.1}%", stats.loss_percent);
    if let (Some(min), Some(max), Some(avg)) = (stats.min, stats.max, stats.avg) {
        println!("Latency: Min={:.2}ms, Max={:.2}ms, Avg={:.2}ms", min, max, avg);
    } else {
        println!("Latency: No data available.");
    }
}

fn read_line() -> String {
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap_or_default();
    input.trim().to_string()
}

fn validate_int<T: std::str::FromStr + PartialOrd + Copy>(prompt: &str, default: Option<T>, min_value: T) -> Option<T> {
    print!("{}", prompt);
    io::stdout().flush().unwrap();
    let input = read_line();
    if input.is_empty() { return default; }
    match input.parse::<T>() {
        Ok(val) if val >= min_value => Some(val),
        _ => { println!("Invalid input, using default."); default }
    }
}

fn validate_float(prompt: &str, default: f64) -> f64 {
    print!("{}", prompt);
    io::stdout().flush().unwrap();
    let input = read_line();
    if input.is_empty() { return default; }
    input.parse::<f64>().unwrap_or_else(|_| { println!("Invalid input, using default {:.1}.", default); default })
}

fn validate_filename(prompt: &str, default: String, format: &str) -> String {
    print!("{}", prompt);
    io::stdout().flush().unwrap();
    let mut name = read_line();
    if name.is_empty() { return default; }
    let extension = match format {
        "csv" => ".csv",
        _ => ".json",
    };
    if !name.to_lowercase().ends_with(extension) { 
        name.push_str(extension); 
    }
    name
}

fn main() -> Result<()> {
    let mut args = Args::parse();

    if args.target.is_none() {
        println!("For help run pinger_rust.exe -h");
        args.target = Some({ print!("Enter host to ping (or Enter to exit): "); io::stdout().flush().unwrap(); read_line() });
        if args.target.as_deref() == Some("") { println!("Exiting."); return Ok(()); }
        
        args.count = validate_int("Number of packets (empty=continuous): ", None, 1);
        args.timeout = validate_float(&format!("Timeout in seconds (default {}): ", args.timeout), args.timeout);
        if let Some(val) = validate_int(&format!("Packet size bytes (default {}): ", args.packet_size), Some(args.packet_size), 0) { args.packet_size = val; }
        
        // Validate format first
        print!("Output format [json/csv] (default json): ");
        io::stdout().flush().unwrap();
        let format_input = read_line();
        if !format_input.is_empty() && (format_input.to_lowercase() == "csv" || format_input.to_lowercase() == "json") {
            args.format = format_input.to_lowercase();
        }
        
        // Update default filename based on format
        let default_filename = match args.format.as_str() {
            "csv" => "ping_history.csv".to_string(),
            _ => "ping_history.json".to_string(),
        };
        args.output = validate_filename(&format!("Results filename (default {}): ", default_filename), default_filename, &args.format);
        
        let dir_str = { print!("Directory to save (default current dir): "); io::stdout().flush().unwrap(); read_line() };
        if !dir_str.is_empty() { args.directory = Some(PathBuf::from(dir_str)); }
        args.save_interval = validate_int("Auto-save interval in seconds (empty=disabled): ", None, 1);
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()?;
    
    runtime.block_on(run_ping(args))
}

async fn run_ping(args: Args) -> Result<()> {
    let target_host = args.target.clone().unwrap();
    let ip_addr = match tokio::net::lookup_host(format!("{}:0", target_host)).await?.next() {
        Some(addr) => match addr.ip() {
            std::net::IpAddr::V4(ip) => ip,
            std::net::IpAddr::V6(_) => return Err(anyhow!("IPv6 is not supported yet.")),
        },
        None => return Err(anyhow!("Could not resolve host.")),
    };

    let save_path = match args.directory {
        Some(dir) => dir.join(&args.output),
        None => PathBuf::from(&args.output),
    };

    // Ensure the output file has the correct extension based on format
    let final_save_path = if args.format == "csv" && !save_path.to_string_lossy().ends_with(".csv") {
        save_path.with_extension("csv")
    } else if args.format == "json" && !save_path.to_string_lossy().ends_with(".json") {
        save_path.with_extension("json")
    } else {
        save_path
    };

    // Shared state for the current session.
    let session_stats = Arc::new(Mutex::new(PingStats::new(ip_addr.to_string())));
    let session_times = Arc::new(Mutex::new(Vec::<f32>::new()));

    println!("Pinging {}...", ip_addr);

    // Set up Ctrl+C handler
    let stats_clone_ctrlc = Arc::clone(&session_stats);
    let times_clone_ctrlc = Arc::clone(&session_times);
    let save_path_clone_ctrlc = final_save_path.clone();
    ctrlc::set_handler(move || {
        println!("\nInterrupted by user. Saving results...");
        let mut stats = stats_clone_ctrlc.lock().unwrap();
        let times = times_clone_ctrlc.lock().unwrap();
        stats.calculate(&times); // Final calculation before saving
        if let Err(e) = save_results_generic(&stats, &save_path_clone_ctrlc) {
            eprintln!("Failed to save results on exit: {}", e);
        } else {
            println!("Results saved to {}", save_path_clone_ctrlc.display());
        }
        print_current_results(&stats);
        std::process::exit(0);
    })?;

    let client = Client::new(&Config::default())?;
    let ident = PingIdentifier(rand::random());
    let mut pinger = client.pinger(std::net::IpAddr::V4(ip_addr), ident).await;
    pinger.timeout(Duration::from_secs_f64(args.timeout));

    let mut last_save_time = tokio::time::Instant::now();
    let save_interval_duration = args.save_interval.map(Duration::from_secs);

    // State for each auto-save interval
    let mut interval_times = Vec::<f32>::new();
    let mut interval_sent: u64 = 0;

    let mut seq: u16 = 0;
    let packets_to_send = args.count.unwrap_or(u64::MAX);

    for i in 0..packets_to_send {
        // Increment counters for both the overall session and the current interval
        session_stats.lock().unwrap().sent += 1;
        interval_sent += 1;

        match pinger.ping(PingSequence(seq), &vec![0; args.packet_size]).await {
            Ok((_, dur)) => {
                let ms = dur.as_secs_f32() * 1000.0;
                println!("Reply from {}: icmp_seq={} time={:.2}ms", ip_addr, i, ms);
                // Record time for both session and interval
                session_times.lock().unwrap().push(ms);
                interval_times.push(ms);
            }
            Err(e) => { println!("Request timed out or error: {}", e); }
        }
        seq = seq.wrapping_add(1);

        // Auto-save logic for the interval
        if let Some(interval) = save_interval_duration {
            if last_save_time.elapsed() >= interval {
                // Create a new stats object specifically for the interval
                let mut interval_stat = PingStats::new(ip_addr.to_string());
                interval_stat.sent = interval_sent;
                interval_stat.calculate(&interval_times);

                if let Err(e) = save_results_generic(&interval_stat, &final_save_path) {
                    eprintln!("Failed to auto-save results: {}", e);
                } else {
                    println!("\n--- Auto-saved interval results to {} ---\n", final_save_path.display());
                }
                
                // Reset interval-specific state
                interval_times.clear();
                interval_sent = 0;
                last_save_time = tokio::time::Instant::now();
            }
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    // This part is reached only if the loop finishes (i.e., `count` was specified).
    let mut stats_guard = session_stats.lock().unwrap();
    let times_guard = session_times.lock().unwrap();
    stats_guard.calculate(&times_guard);

    if stats_guard.sent > 0 {
        if let Err(e) = save_results_generic(&stats_guard, &final_save_path) {
            eprintln!("Failed to save final results: {}", e);
        } else {
            println!("Final results saved to {}", final_save_path.display());
        }
        print_current_results(&stats_guard);
    }

    Ok(())
}
