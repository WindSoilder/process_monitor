use anyhow::Result;
use clap::Parser;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::{fs::OpenOptions, io::Write, path::Path, time::Duration};
use sysinfo::{Pid, Process, ProcessExt, System, SystemExt};

#[derive(Parser)]
struct Args {
    pid: usize,
    output: String,
}

#[derive(Debug)]
struct ProcessStatus {
    memory_max: u64,
    memory_usage: Vec<u64>,
    cpu_usage: Vec<f32>,
    cpu_max: f32,
}

impl ProcessStatus {
    pub fn new() -> Self {
        ProcessStatus {
            memory_max: 0,
            memory_usage: vec![],
            cpu_max: 0.0,
            cpu_usage: vec![],
        }
    }

    pub fn update_info(&mut self, mem: u64, cpu: f32) {
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
        file.write_all(format!("Memory Max: {}\n", self.memory_max,).as_bytes())?;
        file.write_all(format!("Cpu Max: {}\n", self.cpu_max).as_bytes())?;
        let mem_usages: Vec<String> = self.memory_usage.iter().map(|x| x.to_string()).collect();
        file.write_all(mem_usages.join(",").as_bytes())?;
        file.write_all("\n".as_bytes())?;
        let cpu_usages: Vec<String> = self.cpu_usage.iter().map(|x| x.to_string()).collect();
        file.write_all(cpu_usages.join(",").as_bytes())?;
        file.write_all("\n".as_bytes())?;
        Ok(())
    }
}

fn main() -> Result<()> {
    let opts: Args = Args::parse();
    let mut status = ProcessStatus::new();

    // set ctrl-c handler.
    let should_exit = Arc::new(AtomicBool::new(false));
    let should_exit_clone = should_exit.clone();
    ctrlc::set_handler(move || {
        should_exit_clone.store(true, Ordering::SeqCst);
    })
    .expect("Error setting ctrl-c handler");
    let system = System::new();
    loop {
        if should_exit.load(Ordering::SeqCst) {
            status.output(opts.output.clone())?;
            break;
        }
        match system.process(Pid::from(opts.pid)) {
            Some(proc) => run_one_circle(proc, &mut status),
            None => {
                status.output(opts.output.clone())?;
                break;
            }
        };
    }
    Ok(())
}

fn run_one_circle(proc: &Process, status: &mut ProcessStatus) {
    let prev_cpu_usage = proc.cpu_usage();
    // record process information every second.
    std::thread::sleep(Duration::from_secs(1));
    let mem_result = proc.memory();
    let cur_cpu_usage = proc.cpu_usage();
    status.update_info(mem_result, cur_cpu_usage - prev_cpu_usage);
}
