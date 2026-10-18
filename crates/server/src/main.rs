//! `holiday-server [--listen=ADDR]`: the signaling server of the port's
//! network games (`docs/networking.md`), on `0.0.0.0:3536` by default.
//! Players point the game at it with `--signal=ws://HOST:3536`.

use std::net::SocketAddr;

const USAGE: &str = "\
usage: holiday-server [--listen=ADDR]

The signaling server for Holiday Island's network games. Players find
each other here; their games then talk directly (or through TURN).

  --listen=ADDR   the address and port to listen on (default 0.0.0.0:3536)
  --help          this
";

fn main() {
    let mut addr = SocketAddr::from(([0, 0, 0, 0], server::PORT));
    for arg in std::env::args().skip(1) {
        if let Some(a) = arg.strip_prefix("--listen=") {
            addr = a.parse().unwrap_or_else(|e| {
                eprintln!("--listen={a}: {e}");
                std::process::exit(2);
            });
        } else if arg == "--help" || arg == "-h" {
            print!("{USAGE}");
            return;
        } else {
            eprint!("unknown option {arg}\n{USAGE}");
            std::process::exit(2);
        }
    }
    let runtime = tokio::runtime::Runtime::new().unwrap_or_else(|e| {
        eprintln!("no runtime: {e}");
        std::process::exit(1);
    });
    let mut server = server::server(addr);
    match server.bind() {
        Ok(bound) => eprintln!("listening on ws://{bound}"),
        Err(e) => {
            eprintln!("can't listen on {addr}: {e}");
            std::process::exit(1);
        }
    }
    let result = runtime.block_on(async {
        tokio::select! {
            r = server.serve() => r.map_err(|e| e.to_string()),
            _ = tokio::signal::ctrl_c() => Ok(()),
        }
    });
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
