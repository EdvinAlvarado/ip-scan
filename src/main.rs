use ipnet::Ipv4AddrRange;
use main_error::MainError;
use std::net::Ipv4Addr;
use std::process::Command;
use std::thread;
use thiserror::Error;

#[derive(Error, Debug)]
enum ScanError {
    #[error("Missing Arguments")]
    MissingParameter,
    #[error("Ping command incorrect or no ping exe available.")]
    PingCommandError(#[from] std::io::Error),
}

fn ping(ip: Ipv4Addr, count: Option<u32>) -> Result<bool, ScanError> {
    let mut cmd = Command::new("ping");
    cmd.arg(ip.to_string());
    if let Some(c) = count {
        cmd.args(["-n", &c.to_string()]);
    }

    let exit_code = cmd.output().map_err(ScanError::PingCommandError)?.status;
    Ok(exit_code.success())
}

macro_rules! ping {
    ($a: expr) => {
        ping($a, None)
    };
    ($a: expr, $b: expr) => {
        ping($a, $b)
    };
}

fn main() -> Result<(), MainError> {
    let mut args = std::env::args();
    args.next();
    let from = args.next().ok_or(ScanError::MissingParameter)?.parse()?;
    let to = args.next().ok_or(ScanError::MissingParameter)?.parse()?;

    let mut threads = vec![];
    let ips = Ipv4AddrRange::new(from, to);
    for ip in ips {
        let t = thread::spawn(move || {
            if ping!(ip).unwrap() {
                println!("{}", ip);
                return Some(ip);
            }
            None
        });
        threads.push(t);
    }
    let _answered_ips: Vec<Ipv4Addr> = threads
        .into_iter()
        .map(|t| t.join().unwrap())
        .filter_map(|o| o)
        .collect();

    Ok(())
}
