use std::sync::Arc; 
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::runtime::Builder;
use paho_mqtt::{AsyncClient, AsyncReceiver, CreateOptionsBuilder, ConnectOptionsBuilder, Message};

mod cli_args;

use a_3::log;
use a_3::write_log;
use a_3::log::set_tag;
use a_3::publisher::Publisher;
use cli_args::CLI_ARGS;

fn main() {
    let main_rt = Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build().unwrap();

    set_tag("C");

    let mut publisher: Option<Publisher> = None;
    let mut publisher_handle: Option<JoinHandle<()>> = None;
    let mut graceful_stop: Option<Arc<Mutex<bool>>> = None;

    let host_uri = &CLI_ARGS.target_host_uri;

    write_log!("Starting controller client... [host uri={}]", host_uri);

    let mut controller_client = AsyncClient::new(
        CreateOptionsBuilder::new()
        .client_id("controller")
        .server_uri(host_uri)
        .finalize()
    ).unwrap();

    let mut qos: Option<i32> = None;
    let mut delay: Option<u64> = None;
    let mut instancecount: Option<usize> = None;

    main_rt.block_on(async {
        let mut resp_stream: AsyncReceiver<Option<Message>> = controller_client.get_stream(32);

        controller_client.connect(
            ConnectOptionsBuilder::new()
            .user_name("user")
            .password("123")
            .finalize()
        ).await.unwrap();

        write_log!("Controller connected.");

        controller_client.subscribe_many(&[
            "request/qos", "request/delay", "request/instancecount", "request/reset", "request/killall"
        ], &[2, 2, 2, 2, 2]).await.unwrap();

        write_log!("Controller subscribed.\n");

        let mut reset = false;

        while let Ok(Some(resp_msg)) = resp_stream.recv().await {
            println!(r#"Message received. [
    topic={},
    payload={},
    qos={},
]
            "#, resp_msg.topic(), resp_msg.payload_str(), resp_msg.qos());

            match resp_msg.topic() {
                "request/qos" => {
                    // write_log!("Message from topic qos received. [\n    payload={},\n]\n", resp_msg.payload_str());
                    if let Ok(new_qos) = resp_msg.payload_str().parse::<i32>() {
                        qos = Some(new_qos);
                    }
                }
                "request/delay" => {
                    // write_log!("Message from topic delay received. [\n    payload={},\n]\n", resp_msg.payload_str());
                    if let Ok(new_delay) = resp_msg.payload_str().parse::<u64>() {
                        delay = Some(new_delay);
                    }
                }
                "request/instancecount" => {
                    // write_log!("Message from topic instancecount received. [\n    payload={},\n]\n", resp_msg.payload_str());
                    if let Ok(new_count) = resp_msg.payload_str().parse::<usize>() {
                        instancecount = Some(new_count);
                    }
                }
                "request/reset" => { 
                    if graceful_stop.is_some() {
                        write_log!("Terminating publisher clients...\n");
                        *graceful_stop.as_mut().unwrap().lock().await = true;
                    }
                    graceful_stop = None;
                    reset = !reset; 
                }
                "request/killall" => {
                    if graceful_stop.is_some() {
                        write_log!("Terminating publisher clients...\n");
                        *graceful_stop.as_mut().unwrap().lock().await = true;
                        if publisher_handle.is_some() {
                            publisher_handle.as_mut().unwrap().await.unwrap();
                        }
                    }
                    return
                }
                _ => {}
            }

            if reset {
                if let (Some(new_qos), Some(new_delay), Some(new_count)) = 
                    (qos.as_ref(), delay.as_ref(), instancecount.as_ref()) {
                    write_log!("Parameter updated, preparing new publisher task. [\n    qos={}\n    delay={}\n    instancecount={}\n]\n", new_qos, new_delay, new_count);

                    if graceful_stop.is_some() {
                        write_log!("Terminating publisher clients...\n");
                        *graceful_stop.as_mut().unwrap().lock().await = true;
                        if publisher_handle.is_some() {
                            publisher_handle.as_mut().unwrap().await.unwrap();
                        }
                    }

                    if let Ok(new_publisher) = Publisher::connect(
                        host_uri, Some(("user", "123")),
                        new_count.clone(), new_delay.clone(), new_qos.clone()
                    ).await {
                        publisher = Some(new_publisher);
                        graceful_stop = Some(Arc::clone(&(publisher.as_ref().unwrap()).make_graceful_stop));

                        write_log!("Starting new publisher clients...\n");

                        let new_instancecount = new_count.clone();

                        publisher_handle = Some(main_rt.spawn(async move {
                            let stats = publisher.unwrap().start().await;
                            let total_n_messages = stats.iter().sum::<u64>();
                            write_log!(r#"Publisher clients 0..{} terminated. [
    total number of messages sent={}
]
                            "#, new_instancecount, total_n_messages);
                        }));
                    } else {
                        publisher = None;
                        graceful_stop = None;
                    }
                }
            }
        }
    });
}