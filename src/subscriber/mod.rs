use chrono::DateTime;
use chrono::Utc;
use futures::future;
use paho_mqtt::Message;
use tokio::{sync::Mutex, time::sleep};
use std::{sync::Arc, time::Duration, error::Error};
use paho_mqtt::{AsyncClient, AsyncReceiver, ConnectOptionsBuilder, CreateOptionsBuilder};

use crate::log;
use crate::write_log;

pub struct Subscriber {
    clients: Arc<Mutex<Vec<AsyncClient>>>,
    target_qos: i32, target_delay: u64,
    pub make_graceful_stop: Arc<Mutex<bool>>,
}

impl Subscriber {
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
                        .client_id(format!("subscriber_{}", i))
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
                target_qos: qos,
                target_delay: delay,
                make_graceful_stop: Arc::new(Mutex::new(false)),
            });
        } else {
            return Err("".to_string())
        }
    }

    pub async fn start(&mut self) -> Vec<(u64, u64, u64, u64, u64)>{
        let mut async_instances = Vec::new();

        for i in 0..self.clients.lock().await.len() {
            let make_graceful_stop = Arc::clone(&self.make_graceful_stop);

            let clients = Arc::clone(&self.clients);

            let qos = self.target_qos;
            let delay = self.target_delay;
            
            async_instances.push(async move {
                let resp_stream = (*clients.lock().await)[i].get_stream(32);

                let mut delays: Vec<u64> = Vec::new();
                let mut total_runtime: u64 = 0;
                let mut total_n_messages: u64 = 0;
                let mut out_of_order_counter: u64 = 0;
                let mut out_of_order_misses: Vec<u64> = Vec::new();

                (*clients.lock().await)[i].subscribe(
                    format!("counter/{}/{}/{}", i, qos, delay), qos
                ).await.unwrap();

                let mut iter: u64 = 0;
                let mut time_start: Option<DateTime<Utc>> = None;
                let mut delay_counter: Option<DateTime<Utc>> = None;
                while !(*make_graceful_stop.lock().await) {
                    if let Ok(Some(resp_msg)) = resp_stream.recv().await {
                        let time_now = Utc::now();
                        if let Ok(counter) = resp_msg.payload_str().parse::<u64>() {
                            if time_start.is_some() {
                                total_runtime += time_now.signed_duration_since(time_start.unwrap()).num_milliseconds() as u64;
                            }
                            if iter < counter {
                                out_of_order_counter += 1;
                                
                                let missings = (iter..counter).collect::<Vec<u64>>();
                                out_of_order_misses.extend(missings);
                                
                                total_n_messages += 1;
                                iter = counter + 1;
                                delay_counter = None;
                                time_start = Some(Utc::now());
                            } else if iter > counter {
                                if out_of_order_misses.contains(&counter) {
                                    out_of_order_misses.retain(|&x| {x != counter});
                                    total_n_messages += 1;
                                    iter += 1;
                                    delay_counter = None;
                                    time_start = Some(Utc::now());
                                }
                            } else {
                                if delay_counter.is_some() {
                                    delays.push(time_now.signed_duration_since(delay_counter.unwrap()).num_milliseconds() as u64);
                                }
                                total_n_messages += 1;
                                iter += 1;
                                delay_counter = Some(Utc::now());
                                time_start = Some(Utc::now());
                            }
                        }
                    }
                }

                (*clients.lock().await)[i].disconnect(None).await;

                delays.sort();
                let total_delays = delays.iter().sum::<u64>();
                let mean_delay = if delays.is_empty() { 0 } else  { total_delays / delays.len() as u64 };
                let median_delay = if delays.is_empty() { 0 } else { delays[delays.len() / 2] };
                let n_out_of_order_misses = out_of_order_misses.len();

                write_log!(r#"Subscriber client {} terminated. [graceful stop=true] [
    mean_delays={}ms,
    median_delays={}ms,
    messages_per_second={},
    total_number_of_messages={},
    out_of_order_counter={},
    n_out_of_order_missings={},
    target_qos={},
    target_delay={},
]
                    "#, i, 
                    if delays.is_empty() { "NA".to_string() } else { mean_delay.to_string() },
                    if delays.is_empty() { "NA".to_string() } else { median_delay.to_string() },
                    if total_runtime != 0 { (total_n_messages * 1000) / total_runtime } else { 0 }, 
                    total_n_messages, 
                    out_of_order_counter, 
                    n_out_of_order_misses,
                    qos,
                    delay
                );

                return (
                    mean_delay, 
                    if total_runtime != 0 { (total_n_messages * 1000) / total_runtime } else { 0 }, 
                    total_n_messages, 
                    out_of_order_counter, 
                    n_out_of_order_misses as u64
                )
            });
        }
        write_log!(r#"Subscriber clients 0..{} ready. [
    target_qos={},
    target_delay={},
]
        "#, async_instances.len() - 1, self.target_qos, self.target_delay);

        return future::join_all(async_instances).await
    }
}

// Inapplicable for now
pub struct SysSubscriber {
    client: AsyncClient
}

impl SysSubscriber {
    pub async fn connect(host_uri: &str, creds: Option<(&str, &str)>) -> Result<Self, String> {

        return Ok(Self {
            client: {
                let client = AsyncClient::new(
                    CreateOptionsBuilder::new()
                    .client_id("SYS-subscriber".to_string())
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
                client
            }
        })
    }

    pub async fn start(&mut self) {
        let mut resp_stream: AsyncReceiver<Option<Message>> = self.client.get_stream(512);

        self.client.subscribe("$SYS", 2).await.unwrap();

        write_log!("SYS subscriber initiated.");
        println!("SYS subscriber initiated.");

        // This function is unfinished
        todo!();

        while let Ok(Some(resp_msg)) = resp_stream.recv().await {
            println!(r#"Message received from SYS. [
    topic={},
    payload={},
    qos={},
]
        "#, resp_msg.topic(), resp_msg.payload_str(), resp_msg.qos());

            // match resp_msg.topic() {

            // }
        }
    }
}