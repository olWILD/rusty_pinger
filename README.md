Usage:
Linux: sudo ./rusty_pinger
CMD: rusty_pinger.exe
Powershell: .\rusty_pinger or  .\rusty_pinger.exe (.exe is compiled on windows) 

Usage: rusty_pinger.exe [OPTIONS] [TARGET]

Arguments:
  [TARGET]  Target host or IP

Options:
  -c, --count <COUNT>                  Packets to send (default: continuous)
  -t, --timeout <TIMEOUT>              Timeout per ping in seconds [default: 4]
  -s, --packet-size <PACKET_SIZE>      ICMP payload size [default: 56]
  -o, --output <o>                Output file [default: ping_history.json]
  -f, --format <FORMAT>                Output format: json or csv [default: json]
  -d, --directory <DIRECTORY>          Output directory (default: current dir)
      --save-interval <SAVE_INTERVAL>  Interval in seconds to save results automatically
  -h, --help                           Print help
  -V, --version                        Print version

rusty_pinger is CLI app making ping request main difference from standard tools is .json and .csv format output

## Output Formats

### JSON Format (default)
Results are saved as a JSON array with detailed ping statistics, latency buckets, and timestamp information.

### CSV Format  
Results are saved in CSV format with the following columns:
- target, timestamp, sent, received, loss_percent, min, max, avg
- Latency buckets: 0-50ms, 50-100ms, 100-150ms, 150-200ms, 200-250ms, 250-300ms, 300-350ms, 350-400ms, 400-450ms, 450-500ms, 500-999ms, >1000ms

All latency values (min, max, avg) are formatted to 2 decimal places.

## Examples

```bash
# Save results as JSON (default)
rusty_pinger -c 10 8.8.8.8

# Save results as CSV
rusty_pinger -c 10 -f csv -o results.csv 8.8.8.8

# Continuous ping with auto-save every 30 seconds
rusty_pinger --save-interval 30 -f csv 8.8.8.8
```