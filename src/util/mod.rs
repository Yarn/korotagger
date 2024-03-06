
use std::fmt::Debug;
use anyhow::{ Error, anyhow };
use chrono::Duration;

// use discord_lib::serde_json;

pub trait Anyway<T, E: Debug> {
    fn anyway(self) -> Result<T, Error>;
}

impl<T, E: Debug> Anyway<T, E> for Result<T, E> {
    fn anyway(self) -> Result<T, Error> {
        self.map_err(|e| anyhow!("{:?}", e))
    }
}

pub fn parse_json_print_err<'a, T: serde::Deserialize<'a>>(input: &'a str) -> serde_json::Result<T> {
    serde_json::from_str(input)
        .map_err(|err| {
            // dbg!(err.line());
            println!("{}", err);
            
            if let Some(line) = input.lines().skip(err.line()-1).next() {
                let col = err.column();
                let start = ((col as isize) - 5).max(0) as usize;
                let end = (col + 500).min(line.len());
                
                fn find_char_boundary(s: &str, i: usize) -> usize {
                    let mut bound = i;
                    while !s.is_char_boundary(bound) {
                        bound -= 1;
                    }
                    bound
                }
                let start = find_char_boundary(line, start);
                let end = find_char_boundary(line, end);
                
                let sub_str = &line[start..end];
                let arrow = "     ^";
                // dbg!(sub_str);
                println!("{}", sub_str);
                println!("{}", arrow);
            } else {
                println!("Invalid line number.");
            }
            
            err
        })
}

pub fn seconds_f64(d: f64) -> Duration {
    Duration::microseconds((d / 1000000.0) as i64)
}
