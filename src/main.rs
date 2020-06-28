use anyhow::Result;
use async_channel::Receiver;
use clap::Clap;
use heim::process;
use heim::units::{information, Information};
use smol::Task;
use std::time::Duration;

#[derive(Clap)]
struct Opts {
    pid: i32,
}

#[derive(Debug)]
struct ProcessStatus {
    memory_max: Information,
    cpu_max: Information,
}

impl ProcessStatus {
    pub fn new() -> Self {
        ProcessStatus {
            memory_max: Information::new::<information::byte>(0),
            cpu_max: Information::new::<information::byte>(0),
        }
    }

    pub fn update_mem(&mut self, mem: Information) {
        if mem > self.memory_max {
            self.memory_max = mem
        }
    }
}

async fn collect_result(receiver: Receiver<ProcessStatus>) {
    if let Ok(evt) = receiver.recv().await {
        println!("{:?}", evt);
    }
}

fn main() -> Result<()> {
    let opts: Opts = Opts::parse();
    let (sender, receiver) = async_channel::unbounded();

    // get relative process.
    smol::run(async {
        let collector = Task::spawn(collect_result(receiver));

        let proc = process::get(opts.pid).await?;
        let mut status = ProcessStatus::new();
        loop {
            // record every second.
            std::thread::sleep(Duration::from_secs(1));
            let mem_result = proc.memory().await;
            match mem_result {
                Ok(mem) => {
                    let current_usage = mem.rss();
                    println!("{:?}", current_usage);

                    status.update_mem(current_usage);
                }
                Err(_) => {
                    println!("send to others");
                    sender.send(status).await.unwrap();
                    collector.await;
                    break;
                }
            }
        }
        Ok(())
    })
}
