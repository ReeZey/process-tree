use std::{fs::{self, File}, path::PathBuf, process::Command, thread};

fn main() {

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
        
        let mut log_file = log_path.join(file.file_name().unwrap());
        log_file.set_extension("log");

        let thread = thread::spawn(move || {
            let log = File::create(log_file).expect("failed to open log");

            let error_log = log.try_clone().expect("could not clone log");
            
            
            let mut command = Command::new(file)
                .stdout(log)
                .stderr(error_log)
                .spawn()
                .expect("Failed to start process");

            command.wait().unwrap();
        });

        threads.push(thread);
    }

    loop {
        let mut dead_threads = vec![];
        for (idx, thread) in threads.iter().enumerate() {
            if thread.is_finished() {
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

