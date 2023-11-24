use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use paho_mqtt::{AsyncReceiver, QOS_1, QOS_2};
use paho_mqtt::{AsyncClient, client, CreateOptionsBuilder, ConnectOptionsBuilder, Message, Client};
use tokio::{runtime::{Runtime, Builder}, spawn, sync::Mutex};

mod cli_args;

use mqtt_playground::log::{self, set_tag};
use mqtt_playground::write_log;
use mqtt_playground::subscriber::{Subscriber, SysSubscriber};
use cli_args::CLI_ARGS;

fn main() {
    let main_rt = Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build().unwrap();

    set_tag("A");

    let mut subscriber: Option<Subscriber> = None;
    let mut graceful_stop: Option<Arc<Mutex<bool>>> = None;

    let host_uri = &CLI_ARGS.target_host_uri;

    write_log!("Starting analyser client... [host uri={}]\n", host_uri);

    let mut analyser_client = AsyncClient::new(
        CreateOptionsBuilder::new()
        .client_id("analyser")
        .server_uri(host_uri)
        .finalize()
    ).unwrap();

    main_rt.block_on(async {

        analyser_client.connect(
            ConnectOptionsBuilder::new()
            .user_name("user")
            .password("123")
            .finalize()
        ).await.unwrap();

        // let mut sys_subscriber = SysSubscriber::connect(&host_uri, None).await.unwrap();

        // let sys_subscriber_handle = main_rt.spawn(async move {
        //     sys_subscriber.start().await;
        // });

        for delay_exp in CLI_ARGS.delay_level_min..(CLI_ARGS.delay_level_max+1) {
            let delay = if delay_exp == 0 { 0 } 
                else { (2 as u64).pow((delay_exp - 1) as u32) };
            analyser_client.publish(
                Message::new("request/delay", delay.to_string(), QOS_2)
            ).await.unwrap();

            for qos in 0..3 {
                analyser_client.publish(
                    Message::new("request/qos", qos.to_string(), QOS_2)
                ).await.unwrap();

                for instancecount in CLI_ARGS.instancecount_min..(CLI_ARGS.instancecount_max+1) {
                    analyser_client.publish(
                        Message::new("request/instancecount", instancecount.to_string(), QOS_2)
                    ).await.unwrap();

                    println!(r#"Parameters updated. [
    qos={},
    delay={},
    instancecount={},
]
                    "#, qos, delay, instancecount);

                    analyser_client.publish(
                        Message::new("request/reset", String::new(), QOS_2)
                    ).await.unwrap();

                    if let Ok(new_subscriber) = Subscriber::connect(
                        host_uri, Some(("user", "123")), 
                        instancecount, delay, qos
                    ).await {
                        subscriber = Some(new_subscriber);
                        graceful_stop = Some(Arc::clone(&(subscriber.as_ref().unwrap()).make_graceful_stop));
            
                        write_log!("Starting new subscriber clients...\n");
                
                        let subscriber_handle = main_rt.spawn(async move {
                            let stats = subscriber.unwrap().start().await;
                            let total_n_messages = stats.iter().map(|x| {x.2}).sum::<u64>();
                            write_log!(r#"Subscriber clients 0..{} terminated. [
    total number of messages received={}
]
                            "#, instancecount, total_n_messages);
                        });
                    }

                    sleep(Duration::from_secs(CLI_ARGS.mrt)).await;

                    analyser_client.publish(
                        Message::new("request/reset", String::new(), QOS_2)
                    ).await.unwrap();

                    if graceful_stop.is_some() {
                        write_log!("Terminating subscriber clients...\n");
                        *graceful_stop.as_mut().unwrap().lock().await = true;
                    }

                    sleep(Duration::from_secs(CLI_ARGS.reset_buffer)).await;

                    subscriber = None;
                    graceful_stop = None;
                }
            }
        }
        
        analyser_client.publish(
            Message::new("request/killall", String::new(), QOS_2)
        ).await.unwrap();

        println!("Analyser finalizing...\n");
    });
}