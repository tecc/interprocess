use {
    super::util::{NameGen, TestResult},
    anyhow::Context,
    interprocess::os::windows::named_pipe::{pipe_mode, PipeListenerOptions, SendPipeStream},
    std::{
        ffi::OsStr,
        io::{self, prelude::*, BufReader},
        sync::{mpsc::Sender, Arc},
    },
};

static MSG: &str = "Hello from client!\n";

pub fn server(name_sender: Sender<String>, num_clients: u32) -> TestResult {
    let (name, listener) = NameGen::new(true)
        .find_map(|nm| {
            let rnm: &OsStr = nm.as_ref();
            let l = match PipeListenerOptions::new()
                .name(rnm)
                .create_recv_only::<pipe_mode::Bytes>()
            {
                Ok(l) => l,
                Err(e) if e.kind() == io::ErrorKind::AddrInUse => return None,
                Err(e) => return Some(Err(e)),
            };
            Some(Ok((nm, l)))
        })
        .unwrap()
        .context("Listener bind failed")?;

    let _ = name_sender.send(name);

    let mut buffer = String::with_capacity(128);

    for _ in 0..num_clients {
        let mut conn = match listener.accept() {
            Ok(c) => BufReader::new(c),
            Err(e) => {
                eprintln!("Incoming connection failed: {e}");
                continue;
            }
        };

        conn.read_line(&mut buffer).context("Pipe receive failed")?;
        assert_eq!(buffer, MSG);

        buffer.clear();
    }

    Ok(())
}
pub fn client(name: Arc<String>) -> TestResult {
    let mut conn = SendPipeStream::<pipe_mode::Bytes>::connect(name.as_str()).context("Connect failed")?;

    conn.write_all(MSG.as_bytes()).context("Pipe send failed")?;
    conn.flush().context("Pipe flush failed")?;

    Ok(())
}
