use clap::Parser;
use ipnet::Ipv4AddrRange;
use main_error::MainError;
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Semaphore;
use std::net::IpAddr;
use std::io::BufRead;
use std::net::ToSocketAddrs;

#[derive(Error, Debug)]
enum ScanError {
    #[error("Wrong Arguments. Check whether input method was mixed.")]
    WrongArguments,
    #[error("Ping command incorrect or no ping exe available.")]
    IoError(#[from] std::io::Error),
    #[error("Output is not utf8.")]
    PingOutputUtf8Error(#[from] std::string::FromUtf8Error),
    #[error("Could not parse IP address.")]
    AddressError(#[from] std::net::AddrParseError),
    #[error("Could not ping")]
    PingError(#[from] ping::Error),
}


/// Scan ips or hostnames to see if pingable.
/// Scan an IP range from to.
/// For hostnames, pipe a list of hostnames/ips or pass a file including lines of hostnames/ips.
#[derive(Parser)]
#[command(author, version, about)]
struct Args {
    /// start IP address
    from: Option<String>,
    /// end IP address
    to: Option<String>,
    /// Read hostnames/ips from this file
    #[arg(short, long)]
    file: Option<PathBuf>,
    /// Flag to read from pipe.
    #[arg(short, long, action=clap::ArgAction::SetTrue)]
    pipe: bool,
}

fn string_to_ipaddr(s: String) -> Result<IpAddr, ScanError> {
    let ip= s.parse();
    match ip {
        Ok(ip) => Ok(ip),
        Err(e) => Ok(s.to_socket_addrs()?.next().ok_or(ScanError::AddressError(e))?.ip()),
    }
}

#[tokio::main]
async fn main() -> Result<(), MainError> {
    let args = Args::parse();
    let hosts: Vec<IpAddr> = match (args.from, args.to, args.file, args.pipe) {
        (Some(from), Some(to), None, false) => Ipv4AddrRange::new(from.parse()?, to.parse()?)
            .map(Into::into)
            .collect(),
        (None, None, Some(file_path), false) => std::fs::read_to_string(file_path)?
            .lines()
            .map(std::string::ToString::to_string)
            .map(string_to_ipaddr)
            .collect::<Result<Vec<IpAddr>, _>>()?,
        (None, None, None, true) => std::io::stdin().lock()
                                                    .lines()
                                                    .map(|rstring| rstring.map_err(ScanError::IoError).and_then(string_to_ipaddr))
            .collect::<Result<Vec<IpAddr>, ScanError>>()?,
        _ => {
            return Err(ScanError::WrongArguments.into());
        }
    };

    let semaphore = Arc::new(Semaphore::new(8)); // Limit to 8 concurrent pings
    let mut tasks = Vec::with_capacity(hosts.len());
    for host in hosts {
        let permit = semaphore.clone().acquire_owned().await?;
        let t = tokio::spawn(async move {
            let _permit = permit; // Keep the permit alive for the duration of the task
            ping::new(host).send()
        });
        tasks.push(t);
    }

    for task in tasks {
        match task.await? {
            Ok(pr) => println!("{}", pr.source),
            Err(ping::Error::IoError { error: _ }) => (),
            Err(e) => eprintln!("Error pinging: {e}"),
        }
    }

    Ok(())
}
