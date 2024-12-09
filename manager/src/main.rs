use clap::Parser;
use std::net::{SocketAddr, TcpStream};

struct Manager {
    store_conns: Vec<Node>,
    reps: usize,
}

struct Node {
    conn: TcpStream,
    status: NodeStatus,
}

enum NodeStatus {
    Running,
    Down,
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct ManagerArgs {
    /// Port of manager node.
    mgr_port: Option<u16>,
    /// List of storage node ports to try to connect to.
    store_ports: Option<Vec<u16>>,
    /// Replication factor.
    reps: usize,
}

#[tokio::main]
async fn main() {
    // Basically, I will build a little REPL that lets me add nodes.
    // Here is where async comes into play, featuring a TUI.
    // The TUI will have three regions:
    // 1. Status of known nodes (updated by heartbeat)
    // 2. Region for logs.
    // 3. Command prompt to add new nodes.
    //
    // The consistent hash ring relates hashes to indices into the array
    // of connections. This is to satisfy Rust's type rules, since indices
    // make no promise that their associated values are available.
    // This also makes me think that caching keys would be smart.
    // We have a heartbeat, so we can always mark a "write-group" as invalid.
    //
    // To be cooler than what I built before, I am going to try to allow nodes to revive
    // themselves.
    // This means the node will commit its state to disk every once in a while.
    // Of course, there are the same challenges involved with normal filesystems.
    // What if a node goes down mid-write? Perhaps we could try logging.

    let args = ManagerArgs::parse();

    // Note: `Vec` allocates lazily, so you can cheaply construct a Vec w/o
    // allocating a backing buffer until you push something.
    let store_conns = args.store_ports.map_or(Vec::new(), |ports| {
        ports
            .into_iter()
            .map(|p| SocketAddr::from(([127, 0, 0, 1], p)))
            .collect()
    });

    println!("Hello, world!");
}
