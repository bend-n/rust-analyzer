use std::{
    io::{self, Read, Stdin, Stdout, Write, stdin, stdout},
    thread,
};

use log::{debug, trace};

use crossbeam_channel::{Receiver, Sender, bounded, unbounded};

use crate::Message;
/// Creates an LSP connection via stdio.
pub fn stdio_transport(
    mut read_from: impl Read + std::io::BufRead + Sync + Send + 'static,
    mut write_to: impl Write + Sync + Send + 'static,
) -> (Sender<Message>, Receiver<Message>, IoThreads) {
    let (writer_sender, writer_receiver) = unbounded::<Message>();
    let writer = thread::Builder::new()
        .name("send to lsp".to_owned())
        .spawn(move || {
            loop {
                let it = writer_receiver.recv().unwrap();
                trace!("sent message {it:#?}");
                let result = it.write(&mut write_to).unwrap();
                result
            }
        })
        .unwrap();
    let (reader_sender, reader_receiver) = bounded::<Message>(0);
    let reader: thread::JoinHandle<Result<(), io::Error>> = thread::Builder::new()
        .name("read from lsp".to_owned())
        .spawn(move || {
            while let Some(msg) = Message::read(&mut read_from)? {
                let is_exit = matches!(&msg, Message::Notification(n) if n.is_exit());
                trace!("received message {msg:#?}");
                if let Err(e) = reader_sender.send(msg) {
                    return Err(io::Error::other(e));
                }

                if is_exit {
                    break;
                }
            }
            Ok(())
        })
        .unwrap();
    let threads = IoThreads { reader, writer };
    (writer_sender, reader_receiver, threads)
}

// Creates an IoThreads
pub(crate) fn make_io_threads(
    reader: thread::JoinHandle<io::Result<()>>,
    writer: thread::JoinHandle<io::Result<()>>,
) -> IoThreads {
    IoThreads { reader, writer }
}

pub struct IoThreads {
    pub reader: thread::JoinHandle<io::Result<()>>,
    pub writer: thread::JoinHandle<io::Result<()>>,
}

impl IoThreads {
    pub fn join(self) -> io::Result<()> {
        match self.reader.join() {
            Ok(r) => r?,
            Err(err) => std::panic::panic_any(err),
        }
        match self.writer.join() {
            Ok(r) => r,
            Err(err) => {
                std::panic::panic_any(err);
            }
        }
    }
}
