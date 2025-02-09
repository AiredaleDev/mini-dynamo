use std::fmt::Display;

use rmp_serde::{from_read, Serializer};
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Message {
    Heartbeat,
    Busy,
    Get { key: String },
    Put { key: String, value: String },
    Found { value: String },
    NotFound,
    DonePut,
}
const MSG_SIZE: usize = std::mem::size_of::<Message>();

pub type Result<T> = std::result::Result<T, Error>;

pub async fn send_msg(conn: &mut TcpStream, msg: Message) -> Result<()> {
    // No variable-length messages for the sake of simplicity.
    // This is a ~5x pessmization in message size (10 vs 48) although it's a mystery why just the
    // tags are 10 bytes long and not 1 byte long. It might be alignment, but 10 bytes is a weird
    // number to choose if that's the case.
    // I might have fun comparing gRPC vs this too. There are gRPC implementations for Rust!
    // I might also be underselling the perf of MessagePack here.
    // TODO: Investigate variable-length encoding. 48 bytes just to say "OK" is stupid.
    let mut ser_buf = [0u8; MSG_SIZE];
    msg.serialize(&mut Serializer::new(&mut ser_buf[..]))?;
    eprintln!("[INFO] Sending '{msg:?}' over the wire.");
    conn.write_all(&ser_buf).await?;
    eprintln!("[INFO] Sent!");

    Ok(())
}

pub async fn recv_msg(conn: &mut TcpStream) -> Result<Message> {
    let mut ser_buf = [0u8; MSG_SIZE];
    eprintln!("[INFO] Trying to read a message.");
    conn.read_exact(&mut ser_buf[..]).await?;
    // Regarding VLE, I tried just passing the conn into this fn but nooo,
    // this TcpStream implements AsyncRead, but serde only understands io::Read.
    // Maybe there's an async version of this? Colored functions smh.
    let message = from_read(&ser_buf[..])?;
    eprintln!("[INFO] Got '{message:?}'");

    Ok(message)
}

// There has to be a better way to handle errors than this...
#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    RMPEncode(rmp_serde::encode::Error),
    RMPDecode(rmp_serde::decode::Error),
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<rmp_serde::encode::Error> for Error {
    fn from(value: rmp_serde::encode::Error) -> Self {
        Self::RMPEncode(value)
    }
}

impl From<rmp_serde::decode::Error> for Error {
    fn from(value: rmp_serde::decode::Error) -> Self {
        Self::RMPDecode(value)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => e.fmt(f),
            Self::RMPEncode(e) => e.fmt(f),
            Self::RMPDecode(e) => e.fmt(f),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs::OpenOptions,
        io::{Read, Seek, Write},
    };

    // Checks if the idea is sound (it is)
    #[test]
    fn serialize_ok() {
        // "lol" in many languages
        let msg = Message::Put {
            key: "jajaja".into(),
            value: "xdroflmaowwwwwwmdrmdrxaxaxaxa".into(),
        };
        let mut ser_buf = Vec::new();
        msg.serialize(&mut Serializer::new(&mut ser_buf))
            .expect("Couldn't serialize...");
        let msg_prime = from_read(&ser_buf[..]).expect("Couldn't deserialize...");
        assert_eq!(msg, msg_prime);
    }

    // Test if we can deserialize messages like this.
    #[test]
    fn dump_to_file_and_retrieve() {
        let mut fake_channel = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(".the_channel")
            .expect("Couldn't make the file.");

        let msgs = [
            Message::Heartbeat,
            Message::Busy,
            Message::Get {
                key: "considerthefollowing".into(),
            },
            Message::Put {
                key: "professionalism".into(),
                value: "mayreflectwell".into(),
            },
            Message::Found {
                value: "nahiwannabegoofy".into(),
            },
            Message::NotFound,
            Message::DonePut,
        ];

        // Even though this encoding claims to have zero-copy deserialization,
        // will deserializing a type with `String`s allocate two heap buffers?
        let mut write_scratch = [0u8; MSG_SIZE];
        let mut read_scratch = [0u8; MSG_SIZE];
        for msg in msgs {
            msg.serialize(&mut Serializer::new(&mut write_scratch[..]))
                .expect("Failed to serialize.");
            fake_channel
                .write_all(&write_scratch)
                .expect("Failed to write to file.");
            fake_channel
                .seek_relative(-(MSG_SIZE as i64))
                .expect("Didn't seek ahead as far as I expected");
            let bytes_read = fake_channel
                .read(&mut read_scratch)
                .expect("Failed to read file.");
            assert_eq!(bytes_read, MSG_SIZE);
            let recvd_msg = from_read(&read_scratch[..]).expect("Failed to parse from file.");
            assert_eq!(msg, recvd_msg);
        }
    }
}
