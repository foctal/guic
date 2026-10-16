use std::{io::{Read, Write}, process::{Command, Stdio}, time::Duration};

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args[0].as_str() {
        "echo" => {
            let mut input = String::new();
            std::io::stdin().read_to_string(&mut input)?;
            for arg in &args[1..] {
                println!("{arg}");
            }
            print!("{input}");
            eprintln!("stderr marker");
            std::process::exit(23);
        }
        "tree" => {
            let depth: u32 = args[2].parse().expect("depth");
            let mut file = std::fs::OpenOptions::new().create(true).append(true).open(&args[1])?;
            writeln!(file, "{}", std::process::id())?;
            file.sync_all()?;
            if depth > 0 {
                let _child = Command::new(std::env::current_exe()?)
                    .args(["tree", &args[1], &(depth - 1).to_string()])
                    .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
                    .spawn()?;
            }
            std::thread::sleep(Duration::from_secs(60));
        }
        _ => panic!("unknown helper mode"),
    }
    Ok(())
}
