use anyhow::Result;
use async_channel::Receiver;
use async_dup::{Arc, Mutex};
use clap::Clap;
use heim::process;
use heim::units::{information, Information};
use smol::Task;
use std::process as StdProcess;
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

async fn collect_result(receiver: Receiver<Arc<Mutex<ProcessStatus>>>) {
    if let Ok(evt) = receiver.recv().await {
        println!("{:?}", evt);
    }
}

fn main() -> Result<()> {
    let opts: Opts = Opts::parse();
    let (sender, receiver) = async_channel::unbounded();
    let status = Arc::new(Mutex::new(ProcessStatus::new()));

    // get relative process.
    smol::run(async {
        let collector = Task::spawn(collect_result(receiver));

        let proc = process::get(opts.pid).await?;

        // set ctrl-c handler.
        let status_clone = status.clone();
        ctrlc::set_handler(move || {
            println!("{:?}", status_clone);
            StdProcess::exit(0);
        })
        .expect("Error setting ctrl-c handler");

        loop {
            // record every second.
            std::thread::sleep(Duration::from_secs(1));
            let mem_result = proc.memory().await;
            match mem_result {
                Ok(mem) => {
                    let current_usage = mem.rss();
                    let mut status = status.lock();
                    status.update_mem(current_usage);
                }
                Err(_) => {
                    sender.send(status).await.unwrap();
                    collector.await;
                    break;
                }
            }
        }
        Ok(())
    })
}
