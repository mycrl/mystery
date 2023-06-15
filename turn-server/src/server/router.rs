use anyhow::Result;
use bytes::Bytes;
use std::{
    collections::HashMap,
    net::SocketAddr,
    net::IpAddr,
    sync::Arc,
};

use tokio::sync::mpsc::{
    channel,
    Receiver,
    Sender,
};

use tokio::{
    net::UdpSocket,
    sync::RwLock,
};

struct UdpProxy {
    v4: UdpSocket,
    v6: UdpSocket,
}

impl UdpProxy {
    async fn new() -> Result<Self> {
        Ok(Self {
            v4: UdpSocket::bind("0.0.0.0:0").await?,
            v6: UdpSocket::bind("[::]:0").await?,
        })
    }

    async fn send(&self, data: &[u8], addr: &SocketAddr) {
        match addr.ip() {
            IpAddr::V4(_) => self.v4.send_to(data, addr).await,
            IpAddr::V6(_) => self.v6.send_to(data, addr).await,
        }
        .expect("there is an error in the udp proxy in tcp!");
    }
}

pub struct Router {
    senders: RwLock<HashMap<SocketAddr, Sender<Bytes>>>,
    udp: UdpProxy,
}

impl Router {
    pub async fn new() -> Result<Arc<Self>> {
        Ok(Arc::new(Self {
            senders: Default::default(),
            udp: UdpProxy::new().await?,
        }))
    }

    pub async fn find(&self, addr: &SocketAddr) -> bool {
        self.senders.read().await.get(addr).is_some()
    }

    pub async fn get_receiver(
        self: &Arc<Self>,
        addr: SocketAddr,
    ) -> Receiver<Bytes> {
        let (sender, mut receiver) = channel(10);

        {
            self.senders.write().await.insert(addr, sender);
        }

        let this = self.clone();
        let (writer, reader) = channel(10);
        tokio::spawn(async move {
            while let Some(bytes) = receiver.recv().await {
                if writer.send(bytes).await.is_err() {
                    this.remove(&addr).await;
                    break;
                }
            }
        });

        reader
    }

    pub async fn send(&self, addr: &SocketAddr, data: &[u8], realy_udp: bool) {
        let mut is_err = false;

        {
            if let Some(sender) = self.senders.read().await.get(addr) {
                if sender.send(Bytes::copy_from_slice(data)).await.is_err() {
                    is_err = true;
                }
            } else {
                if realy_udp {
                    self.udp.send(data, addr).await;
                }
            }
        }

        if is_err {
            self.remove(addr).await;
        }
    }

    async fn remove(&self, addr: &SocketAddr) {
        if let Some(sender) = self.senders.write().await.remove(addr) {
            drop(sender)
        }
    }
}