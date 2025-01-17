use {
    super::util::{NameGen, TestResult},
    anyhow::Context,
    futures::io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    interprocess::os::windows::named_pipe::{
        pipe_mode,
        tokio::{PipeListenerOptionsExt, RecvPipeStream, SendPipeStream},
        PipeListenerOptions,
    },
    std::{convert::TryInto, ffi::OsStr, io, sync::Arc},
    tokio::{sync::oneshot::Sender, task},
};

static MSG: &str = "Hello from server!\n";

pub async fn server(name_sender: Sender<String>, num_clients: u32) -> TestResult {
    async fn handle_conn(mut conn: SendPipeStream<pipe_mode::Bytes>) -> TestResult {
        conn.write_all(MSG.as_bytes()).await.context("Pipe send failed")?;
        drop(conn);

        Ok(())
    }

    let (name, listener) = NameGen::new(true)
        .find_map(|nm| {
            let rnm: &OsStr = nm.as_ref();
            let l = match PipeListenerOptions::new()
                .name(rnm)
                .create_tokio_send_only::<pipe_mode::Bytes>()
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

    let mut tasks = Vec::with_capacity(num_clients.try_into().unwrap());

    for _ in 0..num_clients {
        let conn = match listener.accept().await {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Incoming connection failed: {e}");
                continue;
            }
        };
        let task = task::spawn(handle_conn(conn));
        tasks.push(task);
    }
    for task in tasks {
        task.await
            .context("Server task panicked")?
            .context("Server task returned early with error")?;
    }

    Ok(())
}
pub async fn client(name: Arc<String>) -> TestResult {
    let mut buffer = String::with_capacity(128);

    let mut conn = RecvPipeStream::<pipe_mode::Bytes>::connect(name.as_str())
        .await
        .context("Connect failed")
        .map(BufReader::new)?;

    conn.read_line(&mut buffer).await.context("Pipe receive failed")?;

    assert_eq!(buffer, MSG);

    Ok(())
}
