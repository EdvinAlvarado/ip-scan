use clap::Parser;
use ipnet::Ipv4AddrRange;
use main_error::MainError;
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;
use tokio::process::Command;
use tokio::sync::Semaphore;

#[derive(Error, Debug)]
enum ScanError {
    #[error("Wrong Arguments. Check whether input method was mixed.")]
    WrongArguments,
    #[error("Ping command incorrect or no ping exe available.")]
    PingCommandError(#[from] std::io::Error),
    #[error("Output is not utf8.")]
    PingOutputUtf8Error(#[from] std::string::FromUtf8Error),
    #[error("Output did not return True or False")]
    PingOutputError((String, String)),
}

async fn ping(ip: String) -> Result<Option<String>, ScanError> {
    let test_cmd = format!(
        "Test-NetConnection {ip} | Select -ExpandProperty \"PingSucceeded\"| echo"
    );
    let mut cmd = Command::new("powershell");
    cmd.arg("-Command");
    cmd.arg(test_cmd);

    let raw_output = cmd.output().await?.stdout;
    let output = String::from_utf8(raw_output)?;

    if output.contains("True") {
        Ok(Some(ip))
    } else if output.contains("False") {
        Ok(None)
    } else {
        Err(ScanError::PingOutputError((ip, output)))
    }
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

#[tokio::main]
async fn main() -> Result<(), MainError> {
    let args = Args::parse();
    let hosts: Vec<String> = match (args.from, args.to, args.file, args.pipe) {
        (Some(from), Some(to), None, false) => Ipv4AddrRange::new(from.parse()?, to.parse()?)
            .map(|ip| ip.to_string())
            .collect(),
        (None, None, Some(file_path), false) => std::fs::read_to_string(file_path)?
            .lines()
            .map(std::string::ToString::to_string)
            .collect(),
        (None, None, None, true) => std::io::stdin().lines().collect::<Result<Vec<String>, _>>()?,
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
            ping(host).await
        });
        tasks.push(t);
    }

    for task in tasks {
        match task.await? {
            Ok(Some(host)) => println!("{host}"),
            Ok(None)  => (),
            Err(ScanError::PingOutputError((host, output))) => eprintln!("Error pinging {host}: {output}"),
            Err(e) => eprintln!("Error pinging: {e}"),
        }
    }

    Ok(())
}
