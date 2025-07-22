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
  -o, --output <OUTPUT>                Output JSON file [default: ping_history.json]
  -d, --directory <DIRECTORY>          Output directory (default: current dir)
      --save-interval <SAVE_INTERVAL>  Interval in seconds to save results automatically
  -h, --help                           Print help
  -V, --version                        Print version

rusty_pinger is CLI app making ping request main diffrence from standart tools is .json format output
