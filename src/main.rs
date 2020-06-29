use anyhow::Result;
use async_channel::Receiver;
use async_dup::{Arc, Mutex};
use clap::Clap;
use heim::{
    process::{self, Process},
    units::{information, ratio, Information},
};
use smol::Task;
use std::{fs::OpenOptions, io::Write, path::Path, process as StdProcess, time::Duration};

#[derive(Clap)]
struct Opts {
    pid: i32,
    output: String,
}

#[derive(Debug)]
struct ProcessStatus {
    memory_max: Information,
    memory_usage: Vec<Information>,
    cpu_usage: Vec<f32>,
    cpu_max: f32,
}

impl ProcessStatus {
    pub fn new() -> Self {
        ProcessStatus {
            memory_max: Information::new::<information::byte>(0),
            memory_usage: vec![],
            cpu_max: 0.0,
            cpu_usage: vec![],
        }
    }

    pub fn update_info(&mut self, mem: Information, cpu: f32) {
        self.memory_usage.push(mem);
        self.cpu_usage.push(cpu);
        if mem > self.memory_max {
            self.memory_max = mem
        }
        if cpu > self.cpu_max {
            self.cpu_max = cpu
        }
    }

    pub fn output<T>(&self, f: T) -> Result<()>
    where
        T: AsRef<Path>,
    {
        let mut file = OpenOptions::new().create(true).write(true).open(f)?;
        file.write_all(
            format!(
                "Memory Max: {}\n",
                self.memory_max.get::<information::byte>()
            )
            .as_bytes(),
        )?;
        file.write_all(format!("Cpu Max: {}\n", self.cpu_max).as_bytes())?;
        let mem_usages: Vec<String> = self
            .memory_usage
            .iter()
            .map(|x| x.get::<information::byte>().to_string())
            .collect();
        file.write_all(mem_usages.join(",").as_bytes())?;
        file.write_all("\n".as_bytes())?;
        let cpu_usages: Vec<String> = self.cpu_usage.iter().map(|x| x.to_string()).collect();
        file.write_all(cpu_usages.join(",").as_bytes())?;
        file.write_all("\n".as_bytes())?;
        Ok(())
    }
}

async fn collect_result(receiver: Receiver<Arc<Mutex<ProcessStatus>>>, result_file: String) {
    if let Ok(evt) = receiver.recv().await {
        let _ = evt.lock().output(result_file);
    }
}

fn main() -> Result<()> {
    let opts: Opts = Opts::parse();
    let (sender, receiver) = async_channel::bounded(1);
    let status = Arc::new(Mutex::new(ProcessStatus::new()));

    smol::run(async {
        // get relative process.
        let collector = Task::spawn(collect_result(receiver, opts.output.clone()));
        let proc = process::get(opts.pid).await?;

        // set ctrl-c handler.
        let status_clone = status.clone();
        ctrlc::set_handler(move || {
            let _ = status_clone.lock().output(opts.output.clone());
            StdProcess::exit(0);
        })
        .expect("Error setting ctrl-c handler");

        loop {
            if let Err(_) = run_one_circle(&proc, &status).await {
                sender.send(status).await.unwrap();
                collector.await;
                break;
            }
        }
        Ok(())
    })
}

async fn run_one_circle(proc: &Process, status: &Arc<Mutex<ProcessStatus>>) -> Result<()> {
    let prev_cpu_usage = proc.cpu_usage().await?;
    // record process information every second.
    std::thread::sleep(Duration::from_secs(1));
    let mem_result = proc.memory().await?.rss();
    let cur_cpu_usage = proc.cpu_usage().await?;
    let mut status = status.lock();
    status.update_info(
        mem_result,
        (cur_cpu_usage - prev_cpu_usage).get::<ratio::percent>(),
    );
    Ok(())
}
