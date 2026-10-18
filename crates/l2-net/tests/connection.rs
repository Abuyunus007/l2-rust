#![allow(clippy::unwrap_used, clippy::expect_used)] // test helpers may panic

//! Loopback tests for the connection layer over 127.0.0.1.

use std::sync::Arc;
use std::time::Duration;

use l2_net::{
    CancellationToken, CipherError, ConnectionConfig, Control, FrameCipher, FrameHandler, NoCipher, listen,
    spawn_connection,
};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

/// Toy cipher: XOR with one key byte, plus a trailing check byte so a wrong key is detected.
struct XorCipher(u8);

impl FrameCipher for XorCipher {
    fn encrypt(&mut self, mut payload: Vec<u8>) -> Vec<u8> {
        payload.push(0x5a);
        payload.iter_mut().for_each(|b| *b ^= self.0);
        payload
    }

    fn decrypt(&mut self, body: &mut Vec<u8>) -> Result<(), CipherError> {
        body.iter_mut().for_each(|b| *b ^= self.0);
        match body.pop() {
            Some(0x5a) => Ok(()),
            _ => Err(CipherError("bad check byte".into())),
        }
    }
}

/// Server side: echoes every frame. "SWITCH" answers "ok" with the old key, then switches.
/// "BYE" answers "bye" and closes.
struct Echo {
    switch_to: u8,
}

impl FrameHandler for Echo {
    async fn on_frame(&mut self, body: Vec<u8>, ctl: &mut Control<'_>) {
        match body.as_slice() {
            b"SWITCH" => {
                ctl.send(b"ok".to_vec());
                ctl.set_cipher(Box::new(XorCipher(self.switch_to)));
            }
            b"BYE" => ctl.send_and_close(b"bye".to_vec()),
            _ => ctl.send(body),
        }
    }
}

/// Client side: forwards everything it receives to the test; sends `script` on connect,
/// switching to `switch_to` right after any "SWITCH" frame.
struct Collector {
    script: Vec<Vec<u8>>,
    switch_to: u8,
    received: mpsc::UnboundedSender<Vec<u8>>,
    disconnected: Option<tokio::sync::oneshot::Sender<()>>,
}

impl FrameHandler for Collector {
    async fn on_connect(&mut self, ctl: &mut Control<'_>) {
        for frame in std::mem::take(&mut self.script) {
            let switch = frame == b"SWITCH";
            ctl.send(frame);
            if switch {
                ctl.set_cipher(Box::new(XorCipher(self.switch_to)));
            }
        }
    }

    async fn on_frame(&mut self, body: Vec<u8>, _ctl: &mut Control<'_>) {
        let _ = self.received.send(body);
    }

    async fn on_disconnect(&mut self) {
        if let Some(tx) = self.disconnected.take() {
            let _ = tx.send(());
        }
    }
}

async fn echo_server(key: u8, switch_to: u8) -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(listen(
        listener,
        Arc::new(ConnectionConfig::default()),
        CancellationToken::new(),
        move |_peer| {
            Some((
                Echo { switch_to },
                Box::new(XorCipher(key)) as Box<dyn FrameCipher>,
            ))
        },
    ));
    addr
}

async fn client(
    addr: std::net::SocketAddr,
    key: u8,
    switch_to: u8,
    script: Vec<Vec<u8>>,
) -> (
    l2_net::ConnectionHandle,
    mpsc::UnboundedReceiver<Vec<u8>>,
    tokio::sync::oneshot::Receiver<()>,
) {
    let stream = TcpStream::connect(addr).await.unwrap();
    let (tx, rx) = mpsc::unbounded_channel();
    let (done_tx, done_rx) = tokio::sync::oneshot::channel();
    let handler = Collector {
        script,
        switch_to,
        received: tx,
        disconnected: Some(done_tx),
    };
    let handle = spawn_connection(
        stream,
        Arc::new(ConnectionConfig::default()),
        Box::new(XorCipher(key)),
        handler,
        CancellationToken::new(),
    )
    .unwrap();
    (handle, rx, done_rx)
}

async fn recv(rx: &mut mpsc::UnboundedReceiver<Vec<u8>>) -> Vec<u8> {
    tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("timed out")
        .expect("closed")
}

#[tokio::test]
async fn echo_with_cipher() {
    let addr = echo_server(0x33, 0).await;
    let (handle, mut rx, _) = client(addr, 0x33, 0, vec![]).await;
    assert!(handle.send(b"hello".to_vec()));
    assert_eq!(recv(&mut rx).await, b"hello");
}

#[tokio::test]
async fn thousand_frames_keep_their_order() {
    let addr = echo_server(0x11, 0).await;
    let (handle, mut rx, _) = client(addr, 0x11, 0, vec![]).await;
    for i in 0..1000u32 {
        assert!(handle.send(i.to_le_bytes().to_vec()));
    }
    for i in 0..1000u32 {
        assert_eq!(recv(&mut rx).await, i.to_le_bytes());
    }
}

#[tokio::test]
async fn send_and_close_delivers_last_frame() {
    let addr = echo_server(0x22, 0).await;
    let (handle, mut rx, done) = client(addr, 0x22, 0, vec![]).await;
    assert!(handle.send(b"BYE".to_vec()));
    assert_eq!(recv(&mut rx).await, b"bye");
    tokio::time::timeout(Duration::from_secs(5), done)
        .await
        .expect("not disconnected")
        .unwrap();
    // Wait until the client task has fully shut down its outbox.
    for _ in 0..50 {
        if handle.is_closed() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(!handle.send(b"late".to_vec()), "sends after close must fail");
}

#[tokio::test]
async fn key_switch_server_side_ordering() {
    // The client writes "SWITCH" (old key) and "after" (new key) back to back, without waiting.
    // The server must decrypt "after" with the new key, because frames are handled one at a
    // time. The client reads with a cipher that accepts both keys and tags which one worked,
    // so we can see which key the server used for each reply.
    struct Both {
        old: u8,
        new: u8,
        sending_new: bool,
    }
    impl FrameCipher for Both {
        fn encrypt(&mut self, payload: Vec<u8>) -> Vec<u8> {
            let key = if self.sending_new { self.new } else { self.old };
            XorCipher(key).encrypt(payload)
        }
        fn decrypt(&mut self, body: &mut Vec<u8>) -> Result<(), CipherError> {
            let copy = body.clone();
            if XorCipher(self.old).decrypt(body).is_ok() {
                body.insert(0, b'O');
                return Ok(());
            }
            *body = copy;
            XorCipher(self.new).decrypt(body)?;
            body.insert(0, b'N');
            Ok(())
        }
    }
    struct Script(mpsc::UnboundedSender<Vec<u8>>);
    impl FrameHandler for Script {
        async fn on_connect(&mut self, ctl: &mut Control<'_>) {
            ctl.send(b"SWITCH".to_vec());
            ctl.set_cipher(Box::new(Both {
                old: 0x44,
                new: 0x77,
                sending_new: true,
            }));
            ctl.send(b"after".to_vec());
        }
        async fn on_frame(&mut self, body: Vec<u8>, _ctl: &mut Control<'_>) {
            let _ = self.0.send(body);
        }
    }

    let addr = echo_server(0x44, 0x77).await;
    let stream = TcpStream::connect(addr).await.unwrap();
    let (tx, mut rx) = mpsc::unbounded_channel();
    let _handle = spawn_connection(
        stream,
        Arc::new(ConnectionConfig::default()),
        Box::new(Both {
            old: 0x44,
            new: 0x77,
            sending_new: false,
        }),
        Script(tx),
        CancellationToken::new(),
    )
    .unwrap();
    assert_eq!(recv(&mut rx).await, b"Ook", "reply to SWITCH uses the old key");
    assert_eq!(
        recv(&mut rx).await,
        b"Nafter",
        "the echo of the next frame uses the new key"
    );
}

#[tokio::test]
async fn factory_can_refuse_connections() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(listen(
        listener,
        Arc::new(ConnectionConfig::default()),
        CancellationToken::new(),
        |_peer| None::<(Echo, Box<dyn FrameCipher>)>,
    ));
    let (handle, _rx, done) = {
        let stream = TcpStream::connect(addr).await.unwrap();
        let (tx, rx) = mpsc::unbounded_channel();
        let (done_tx, done_rx) = tokio::sync::oneshot::channel();
        let h = spawn_connection(
            stream,
            Arc::new(ConnectionConfig::default()),
            Box::new(NoCipher),
            Collector {
                script: vec![],
                switch_to: 0,
                received: tx,
                disconnected: Some(done_tx),
            },
            CancellationToken::new(),
        )
        .unwrap();
        (h, rx, done_rx)
    };
    let _ = handle.send(b"anyone?".to_vec());
    tokio::time::timeout(Duration::from_secs(5), done)
        .await
        .expect("refused connection must end")
        .unwrap();
}

#[tokio::test]
async fn cancel_stops_the_listener_and_connections() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let cancel = CancellationToken::new();
    let task = tokio::spawn(listen(
        listener,
        Arc::new(ConnectionConfig::default()),
        cancel.clone(),
        move |_peer| Some((Echo { switch_to: 0 }, Box::new(NoCipher) as Box<dyn FrameCipher>)),
    ));
    let (_handle, _rx, done) = {
        let stream = TcpStream::connect(addr).await.unwrap();
        let (tx, rx) = mpsc::unbounded_channel();
        let (done_tx, done_rx) = tokio::sync::oneshot::channel();
        let h = spawn_connection(
            stream,
            Arc::new(ConnectionConfig::default()),
            Box::new(NoCipher),
            Collector {
                script: vec![],
                switch_to: 0,
                received: tx,
                disconnected: Some(done_tx),
            },
            CancellationToken::new(),
        )
        .unwrap();
        (h, rx, done_rx)
    };
    tokio::time::sleep(Duration::from_millis(100)).await;
    cancel.cancel();
    tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .expect("listener must stop")
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), done)
        .await
        .expect("server side closed, client sees EOF")
        .unwrap();
}
