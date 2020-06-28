use anyhow::Result;
use async_channel::Receiver;
use async_dup::{Arc, Mutex};
use clap::Clap;
use heim::process;
use heim::units::{information, ratio, Information};
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
    cpu_max: f32,
}

impl ProcessStatus {
    pub fn new() -> Self {
        ProcessStatus {
            memory_max: Information::new::<information::byte>(0),
            cpu_max: 0.0,
        }
    }

    pub fn update_info(&mut self, mem: Information, cpu: f32) {
        if mem > self.memory_max {
            self.memory_max = mem
        }
        if cpu > self.cpu_max {
            self.cpu_max = cpu
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
            let prev_cpu_usage = proc.cpu_usage().await;
            // record process information every second.
            std::thread::sleep(Duration::from_secs(1));
            let mem_result = proc.memory().await;
            let cur_cpu_usage = proc.cpu_usage().await;
            match mem_result {
                Ok(mem) => {
                    let current_mem_usage = mem.rss();
                    let cpu_usage = cur_cpu_usage? - prev_cpu_usage?;
                    let mut status = status.lock();
                    status.update_info(current_mem_usage, cpu_usage.get::<ratio::percent>())
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
