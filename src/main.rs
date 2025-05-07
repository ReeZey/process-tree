use std::{fs::{self}, os::unix::process::ExitStatusExt, path::PathBuf, process::{ExitStatus, Stdio}};
use serde::Deserialize;
use tokio::{fs::File, io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader}, process::Command};
use tokio_stream::StreamExt;
use tokio_util::codec::{Framed, FramedRead, LinesCodec};

#[derive(Deserialize, Debug)]
struct Config {
    directory: PathBuf,
    exec: String,
    retry: Option<u32>,
}

#[tokio::main]
async fn main() {

    let processes_path = PathBuf::from("processes");
    let log_path = PathBuf::from("logs");

    if !fs::exists(&log_path).unwrap() {
        fs::create_dir_all(&log_path).unwrap();
    }

    if !fs::exists(&processes_path).unwrap() {
        fs::create_dir_all(&processes_path).unwrap();
    }

    let directory_entries: Vec<PathBuf> = fs::read_dir(&processes_path).unwrap().filter_map(|f| f.ok()).map(|f| f.path()).collect();

    if directory_entries.len() == 0 {
        println!("No processes to run");
        return;
    }

    let mut threads = vec![];
    for file in directory_entries {
        match file.extension() {
            Some(str) => {
                if str == "disabled" {
                    continue;
                }
            },
            None => {}
        };

        let mut current_file = File::open(&file).await.expect("failed to open file");
        let mut contents = String::new();
        current_file.read_to_string(&mut contents).await.expect("failed to read file");
        
        let config: Config = toml::from_str(contents.as_str()).unwrap();

        let retry = config.retry.unwrap_or(5);

        let mut log_file = log_path.join(file.file_name().unwrap());
        log_file.set_extension("log");

        let thread = tokio::spawn(async move {
            let mut log = File::create(log_file).await.expect("failed to open log");

            let mut command = Command::new(config.exec.clone())
                .current_dir(config.directory)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("Failed to start process");

            let stdout = command.stdout.take().unwrap();
            let stderr = command.stderr.take().unwrap();

            let mut stdout_framed = FramedRead::new(stdout, LinesCodec::new());
            let mut stderr_framed = FramedRead::new(stderr, LinesCodec::new());
            
            let exit_status = loop {
                let abc = tokio::time::timeout(tokio::time::Duration::from_millis(100), command.wait()).await;
                match abc {
                    Ok(Ok(status)) => {
                        println!("Process finished: {:?}", config.exec);

                        break status;
                    }
                    Ok(Err(e)) => {
                        println!("Error waiting for process: {:?}", e);

                        break ExitStatus::from_raw(1);
                    }
                    Err(_) => {}
                }

                println!("Process still running: {:?}", config.exec);

                
                
                while let Some(line) = stdout_framed.next().await {
                    match line {
                        Ok(text) => {
                            log.write_all(text.as_bytes()).await.expect("failed to write to log");
                            log.write_all(b"\n").await.expect("failed to write to log");
                        },
                        Err(e) => eprintln!("Error reading: {}", e),
                    }
                }

                while let Some(line) = stderr_framed.next().await {
                    match line {
                        Ok(text) => {
                            log.write_all(text.as_bytes()).await.expect("failed to write to log");
                            log.write_all(b"\n").await.expect("failed to write to log");
                        },
                        Err(e) => eprintln!("Error reading: {}", e),
                    }
                }
            };


            exit_status
        });

        threads.push(thread);
    }

    loop {
        let mut dead_threads = vec![];
        for (idx, thread) in threads.iter_mut().enumerate() {
            if thread.is_finished() {
                let status = thread.await.unwrap();

                println!("Thread finished: {:?}", status);
                dead_threads.push(idx);
            }
        }

        for idx in dead_threads.iter().rev() {
            threads.remove(*idx);
        }

        if threads.len() == 0 {
            println!("done");
            break;
        }
    }
}

