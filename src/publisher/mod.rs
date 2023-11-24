use futures::future;
use paho_mqtt::Message;
use tokio::{sync::Mutex, time::sleep};
use std::{sync::Arc, time::Duration, error::Error};
use paho_mqtt::{AsyncClient, ConnectOptionsBuilder, CreateOptionsBuilder};

use crate::log;
use crate::write_log;

pub struct Publisher {
    clients: Arc<Mutex<Vec<AsyncClient>>>,
    pub make_graceful_stop: Arc<Mutex<bool>>,
    delay: u64, qos: i32,
}

impl Publisher {
    pub async fn connect(
        host_uri: &str, creds: Option<(&str, &str)>,
        n_instances: usize, delay: u64, qos: i32
    ) -> Result<Self, String> {
        if n_instances > 0 {
            let mut connect_all = Vec::new();

            for i in 0..n_instances {
                connect_all.push(async move {
                    let client = AsyncClient::new(
                        CreateOptionsBuilder::new()
                        .client_id(format!("publisher_{}", i))
                        .server_uri(host_uri)
                        .finalize()
                    ).unwrap();
                    if let Some((username, password)) = creds {
                        client.connect(
                            ConnectOptionsBuilder::new()
                            .user_name(username)
                            .password(password)
                            .finalize()
                        ).await.unwrap();
                    } else {
                        client.connect(
                            ConnectOptionsBuilder::new().finalize()
                        ).await.unwrap();
                    }
                    return client
                });
            }
    
            return Ok(Self {
                clients: Arc::new(Mutex::new(
                    future::join_all(connect_all).await
                )),
                make_graceful_stop: Arc::new(Mutex::new(false)),
                delay: delay,
                qos: qos,
            });
        } else {
            return Err("".to_string())
        }
    }

    pub async fn start(&mut self) -> Vec<u64> {
        let mut async_instances = Vec::new();
        for i in 0..self.clients.lock().await.len() {
            let clients = Arc::clone(&self.clients);
            let make_graceful_stop = Arc::clone(&self.make_graceful_stop);
            let delay = self.delay;
            let qos = self.qos;

            async_instances.push(async move {
                let mut iter: u64 = 0;
                while !(*make_graceful_stop.lock().await) {
                    // applying delay
                    sleep(Duration::from_millis(delay)).await;

                    let mut retries = 3;
                    loop {
                        match (*clients.lock().await)[i].publish(
                            Message::new(
                                format!("counter/{}/{}/{}", i, qos, delay), 
                                iter.to_string(), 
                                qos as i32
                            )
                        ).await {
                            Ok(_) => { break; }
                            Err(msg) => { 
                                if retries == 0 { break; }
                                retries -= 1;
                            }
                        }
                    }

                    iter += 1;
                }

                (*clients.lock().await)[i].disconnect(None).await;
                
                write_log!(r#"Publisher client {} terminated. [graceful stop=true] [
    total_n_messages_sent={},
    qos={},
]
                "#, i, iter, qos);

                return iter;
            });
        }

        write_log!(r#"Publisher clients 0..{} ready. [
    qos={},
    delay={},
]
        "#, async_instances.len() - 1, self.qos, self.delay);

        return future::join_all(async_instances).await
    }
}