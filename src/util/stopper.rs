
use tokio::sync::mpsc::{ Sender, Receiver, channel };

#[derive(Debug, Clone)]
pub struct Stopper {
    send: Sender<()>,
}

impl Stopper {
    pub async fn stop(&self) -> Result<(), ()> {
        self.send.send(()).await.map_err(|_| ())
    }
}

#[derive(Debug)]
pub struct Stoppable {
    recv: Receiver<()>,
}

impl Stoppable {
    pub fn new() -> (Stopper, Stoppable) {
        let (send, recv) = channel(1);
        (Stopper { send }, Stoppable { recv })
    }
    
    pub async fn run<T>(&mut self, fut: impl futures::Future<Output = T>) -> Option<T> {
        let res = tokio::select! {
            msg = fut => {
                msg
            }
            _ = self.recv.recv() => {
                return None;
            }
        };
        
        Some(res)
    }
}
