use clap::Parser;
use comm::{recv_msg, send_msg, Message, Result};
use fnv::FnvHashMap;
use rmp_serde::{from_read, Serializer};
use serde::Serialize;
use std::{
    fs::{self, File},
    io::{Seek, SeekFrom, Write},
    net::SocketAddr,
    path::PathBuf,
    sync::{LazyLock, RwLock},
    time::Duration,
};
use tokio::net::{TcpListener, TcpStream};

// TODO: Fix port argument not to collide with default manager port.
#[derive(Parser)]
#[command(version, about, long_about = None)]
struct StoreArgs {
    /// Port of this storage node.
    #[arg(short, long, default_value_t = 50051)]
    port: u16,
    /// File to persist state to. If none is provided, does not persist state.
    #[arg(short, long)]
    node_state: Option<PathBuf>,
}

// TODO: Benchmark this vs Dashmap, which presumably only uses atomics like ConcurrentHashMap from
// Java (wow imagine Java doing anything correctly...)
static TABLE_SERVICE: LazyLock<RwLock<FnvHashMap<String, String>>> =
    LazyLock::new(|| RwLock::new(FnvHashMap::default()));

async fn handle_client(mut conn: TcpStream) {
    match recv_msg(&mut conn).await {
        Ok(msg) => match msg {
            Message::Get { key } => {
                let response = TABLE_SERVICE
                    .read()
                    .expect("Lock poisoned :(")
                    .get(&key)
                    .map_or(Message::NotFound, |val| Message::Found {
                        value: val.clone(),
                    });

                if let Err(e) = send_msg(&mut conn, response).await {
                    eprintln!("[ERROR] Failed to respond to GET request from {conn:?}: {e}");
                }
            }
            Message::Put { key, value } => {
                TABLE_SERVICE
                    .write()
                    .expect("Lock poisoned :(")
                    .insert(key, value);
                if let Err(e) = send_msg(&mut conn, Message::DonePut).await {
                    eprintln!("[ERROR] Failed to respond to PUT request from {conn:?}: {e}");
                }
            }
            _ => unreachable!(),
        },
        Err(e) => {
            eprintln!("[ERROR] Failed to recv msg from {conn:?}: {e}");
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    const TIMEOUT: Duration = Duration::from_secs(2);
    const DISK_SYNC_PERIOD: Duration = Duration::from_secs(10);
    // This guy serves multple connections.
    // So I guess our storage node should listen to one of multiple possibilites.
    // Here is where workspace-shared code comes in: serde + message type

    // 1. Wait for manager heartbeat message.
    //    Respond w/ OK once alive. Reply to all clients that you
    //    are busy while in this state to prevent opportunistic writers
    //    from corrupting your knowledge.
    // 2. Once you've fired back an OK to the manager,
    //    advance to the main request loop, which responds to GET/PUT/HB.

    let args = StoreArgs::parse();

    // Synchronize hash table to disk.
    // I'd like to abstract this out into its own function but this lambda returns a JoinHandle<!>,
    // where the bottom type `!` is "experimental"
    // We have the nightly compiler so ig it's fine.
    // I really just want a semaphore but this'll probably do.
    let (quit_tx, quit_rx) = std::sync::mpsc::channel();
    let sync_handle = args.node_state.map(|path| {
        let mut backing_file = if fs::exists(&path).expect("Bigger FS problem:") {
            eprintln!("[INFO] Attempting to recover node state @ {path:?}");
            let file = File::options()
                .read(true)
                .write(true)
                .open(path)
                .expect("Bigger FS problem:");
            let got_map = from_read(&file).expect("Failed to deserialize:");
            eprintln!("[INFO] Recovered state {got_map:#?}");
            *TABLE_SERVICE.write().expect("Lock already poisoned?!") = got_map;
            file
        } else {
            eprintln!("[INFO] Creating backing file @ {path:?}");
            File::create_new(path).expect("Bigger FS problem:")
        };

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(DISK_SYNC_PERIOD);
            let mut table_buffer = Vec::new();
            loop {
                interval.tick().await;
                {
                    let table_guard = TABLE_SERVICE.read().expect("Lock poisoned :(");
                    table_guard
                        .serialize(&mut Serializer::new(&mut table_buffer))
                        .expect("Failed to serialize table!");
                    backing_file
                        .write_all(&mut table_buffer[..])
                        .expect("Couldn't fully write state!");
                    backing_file
                        .seek(SeekFrom::Start(0))
                        .expect("Couldn't seek.");
                    table_buffer.clear();
                    eprintln!("[INFO] Synced to disk!");
                }

                if let Ok(()) = quit_rx.try_recv() {
                    eprintln!("[INFO] Main storage loop exited! Quitting...");
                    break;
                }
            }
        })
    });

    let addr = SocketAddr::from(([127, 0, 0, 1], args.port));
    let listener = TcpListener::bind(addr).await?;

    eprintln!("[INFO] Listening on {}", args.port);

    // Connections will come in. Hopefully they are the manager. If they are not,
    // we aren't ready for them. Denied.
    // let mut mgr_conn = loop {
    //     let (mut conn, _peer) = listener.accept()?;
    //     conn.set_read_timeout(Some(TIMEOUT))?;
    //     conn.set_write_timeout(Some(TIMEOUT))?;

    //     let msg = recv_msg(&mut conn)?;
    //     match msg {
    //         Message::Heartbeat => {
    //             send_msg(&mut conn, Message::Heartbeat)?;
    //             break conn;
    //         }
    //         Message::Get { .. } | Message::Put { .. } => send_msg(&mut conn, Message::Busy)?,
    //         _ => unreachable!(),
    //     }
    // };

    // tokio::spawn(async move {
    //     while let Ok(msg) = recv_msg(&mut mgr_conn) {
    //         if let Message::Heartbeat = msg {
    //             if let Err(e) = send_msg(&mut mgr_conn, Message::Heartbeat) {
    //                 eprintln!("[ERROR] Couldn't send the heartbeat, the manager likely thinks we're down (or is down itself...): {e}");
    //             }
    //         }
    //     }
    //     eprintln!("[INFO] Exiting mgr_conn loop...");
    // });

    // Now we can handle connections normally.
    let conn_loop = tokio::spawn(async move {
        loop {
            use tokio::time::timeout;
            let (conn, _) = listener
                .accept()
                .await
                .expect("Failed to accept connection!");
            eprintln!("[INFO] Got connection!");
            tokio::spawn(timeout(TIMEOUT, handle_client(conn)));
        }
    });

    // If something horrible goes wrong, ensure the synchronizing loop can finish.
    // We need to signal to the sync loop to shutdown.
    let _ = conn_loop.await;
    if let Some(handle) = sync_handle {
        if let Err(e) = quit_tx.send(()) {
            eprintln!("[WARN] Couldn't send quit signal to disk coroutine because: {e}");
        }
        let _ = handle.await;
    }
    Ok(())
}
