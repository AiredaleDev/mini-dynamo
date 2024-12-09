use clap::{Parser, Subcommand};
use comm::{recv_msg, send_msg, Message, Result};
use std::net::SocketAddr;
use tokio::net::TcpStream;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct ClientArgs {
    /// Port of manager node.
    #[arg(short, long, default_value_t = 50051)]
    mgr_port: u16,
    #[command(subcommand)]
    command: DBRequest,
}

#[derive(Subcommand)]
enum DBRequest {
    /// Get a key from the system
    Get {
        /// Key to fetch.
        key: String,
    },
    /// Put a key-value pair into the system.
    Put {
        /// Key to update.
        key: String,
        /// Value to update key with.
        value: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = ClientArgs::parse();
    let addr = SocketAddr::from(([127, 0, 0, 1], args.mgr_port));

    // First, will a client send multiple requests in its lifetime?
    // For parity, let us assume that it will not.
    // TODO: Bother with manager.
    // eprintln!("[INFO] Connecting to manager @ {}", addr);
    // let manager_stream = TcpStream::connect(addr)?;
    // eprintln!("[INFO] Connected!");
    eprintln!("[INFO] Connecting to storage node");
    let mut store_stream = TcpStream::connect(addr).await?;
    eprintln!("[INFO] Connected!");
    let peer = store_stream.peer_addr()?;

    match args.command {
        DBRequest::Get { key } => {
            // This clone is sort of cringe and unnecessary -- a product of how I chose to
            // implement Message. TODO: Make a nice API that avoids this.
            send_msg(&mut store_stream, Message::Get { key: key.clone() }).await?;
            let response = recv_msg(&mut store_stream).await?;
            match response {
                Message::NotFound => eprintln!("Not Found: {peer}"),
                Message::Found { value } => eprintln!("OK, {key}, {value}, {peer}"),
                _ => unreachable!(),
            }
        }
        DBRequest::Put { key, value } => {
            send_msg(&mut store_stream, Message::Put { key, value }).await?;
            let response = recv_msg(&mut store_stream).await?;
            match response {
                Message::DonePut => eprintln!("OK, {peer}"),
                _ => unreachable!(),
            }
        }
    }

    Ok(())
}
