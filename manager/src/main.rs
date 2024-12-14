use clap::Parser;
use std::net::SocketAddr;
use tokio::net::TcpStream;

mod ring_hash;

struct Manager {
    nodes: Vec<Node>,
    reps: usize,
}

struct Node {
    conn: Option<TcpStream>,
    port: u16,
}

impl Node {
    pub async fn connect_on_port(port: u16) -> Self {
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        let conn = TcpStream::connect(addr).await;
        if let Err(e) = &conn {
            eprintln!("Failed to connect to '{addr:?}': {e}");
        }
        let conn = conn.ok();

        Self { conn, port }
    }

    pub fn is_alive(&self) -> bool {
        self.conn.is_some()
    }
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct ManagerArgs {
    /// Port of manager node.
    #[arg(short, long, default_value_t = 50051)]
    port: u16,
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

    // I had this nice FP-style solution only for async semantics to ruin it.
    // Even though this function is in an async block, the lambdas I passed in also had to be
    // marked as async to call async code, which would cause nodes' type to be `Vec<async block>`
    // even though said block only did effects. Maybe it's because iterators are lazy?
    let nodes = {
        let ports = args.store_ports.unwrap_or(Vec::new());
        let mut nodes = Vec::with_capacity(ports.len());
        for p in ports {
            // We don't return an error if we fail to connect because we figure the user may want to
            // remember this node or try to connect to it as soon as possible.
            nodes.push(Node::connect_on_port(p).await);
        }
        nodes
    };

    let mut mgr_state = Manager {
        nodes,
        reps: args.reps,
    };

    println!("Hello, world!");
}
