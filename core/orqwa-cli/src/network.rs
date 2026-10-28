//! Explicit network choices shared by the developer networking commands.

use anyhow::Result;
use clap::Args;
use orqwa_net::discovery::{self, GatherOptions};
use orqwa_net::policy::AddressPolicy;
use tokio::time::Instant;

#[derive(Args)]
pub(crate) struct NetworkArgs {
    /// Permit private addresses for a deliberate local-network ride.
    #[arg(long, global = true, conflicts_with = "loopback")]
    lan: bool,
    /// Use only this computer's loopback sockets; no STUN or interface discovery.
    #[arg(long,global=true,conflicts_with_all=["lan","interface","stun"])]
    loopback: bool,
    /// OS network interface name; otherwise use the default interface only.
    #[arg(long, global = true)]
    interface: Option<String>,
    /// Ask no public STUN server and send no STUN DNS requests.
    #[arg(long, global = true, conflicts_with = "stun")]
    no_stun: bool,
    /// Use these STUN host:port addresses instead of the defaults (at most four).
    #[arg(long, global = true)]
    stun: Vec<String>,
}

impl NetworkArgs {
    pub(crate) fn options(&self) -> GatherOptions {
        let mut options = GatherOptions {
            policy: AddressPolicy {
                lan: self.lan,
                loopback: self.loopback,
            },
            interface: self.interface.clone(),
            ..GatherOptions::default()
        };
        if self.no_stun || self.loopback {
            options.stun_servers.clear();
        } else if !self.stun.is_empty() {
            options.stun_servers.clone_from(&self.stun);
        }
        options
    }
}

#[derive(Args)]
pub(crate) struct NetCheckArgs {
    #[command(flatten)]
    network: NetworkArgs,
}

pub(crate) async fn check(args: &NetCheckArgs) -> Result<()> {
    let paths = discovery::bind(
        &args.network.options(),
        Instant::now() + discovery::DISCOVERY_WINDOW,
    )
    .await?;
    for path in paths {
        println!("UDP {}", path.socket.local_addr()?);
        if path.candidates.is_empty() {
            println!(
                "  no permitted reachable addresses; try LAN mode, STUN or a public IPv6 interface"
            );
        }
        for candidate in path.candidates {
            println!("  {:?} {}", candidate.kind(), candidate.addr());
        }
        println!("  STUN observation: {:?}", path.nat);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    #[test]
    fn local_and_offline_checks_do_not_inherit_public_discovery_servers() {
        for flag in ["--loopback", "--no-stun"] {
            let cli = crate::Cli::try_parse_from(["orqwa-cli", "net-check", flag]).unwrap();
            let crate::Command::NetCheck(args) = cli.command else {
                panic!("net-check");
            };
            assert_eq!(args.network.options().stun_servers, Vec::<String>::new());
        }
        assert!(
            crate::Cli::try_parse_from(["orqwa-cli", "net-check", "--loopback", "--lan"]).is_err()
        );
        assert!(
            crate::Cli::try_parse_from([
                "orqwa-cli",
                "net-check",
                "--no-stun",
                "--stun",
                "1.1.1.1:3478"
            ])
            .is_err()
        );
    }
}
